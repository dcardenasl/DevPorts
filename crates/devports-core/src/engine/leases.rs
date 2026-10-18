//! Leases: Temporals live for a while, warn before the end and are stopped when
//! it comes. Only things with a lease are ever stopped automatically (ADR-006).

use devports_proto::{ErrorBody, ErrorCode, ExtendResult, StopResult};
use serde_json::json;

use super::{Effect, Engine, Notice, StopReason, Target, Terminated, TerminationRequest};
use crate::clock::Clock;
use crate::ids::token_hash;
use crate::messages;
use crate::ports::PortProbe;
use crate::process::{ProcessId, ProcessSource, ProcessStart};
use crate::reasons::Problem;
use crate::store::{InstanceRecord, Role, State};

/// How often old events are pruned, in seconds.
const PRUNE_EVERY: i64 = 3_600;

impl<C: Clock, S: ProcessSource, P: PortProbe> Engine<C, S, P> {
    /// Advances time: releases reservations nobody used, warns about leases that
    /// are about to end and starts stopping the ones that ended.
    ///
    /// Called every second by the runtime, and once right after start so that
    /// leases that expired while the core was down are stopped at once.
    pub fn tick(&mut self) -> Vec<Effect> {
        let now = self.now();
        let mut effects = self.settle();
        let warn_before =
            i64::try_from(self.config.lease.warn_before.as_secs()).unwrap_or(i64::MAX);

        let candidates: Vec<InstanceRecord> = self
            .tracked
            .iter()
            .filter(|record| record.role == Role::Temporal && record.state == State::Running)
            .cloned()
            .collect();
        let mut changed = false;
        for mut record in candidates {
            let Some(end) = record.expires_at else {
                continue;
            };
            if now >= end {
                changed = true;
                record.state = State::Stopping;
                self.persist(&record);
                self.update_tracked(&record);
                if let Some(target) = target_of(&record) {
                    effects.push(Effect::Terminate(TerminationRequest {
                        reference: record.reference.clone(),
                        target,
                        reason: StopReason::LeaseExpired,
                    }));
                }
            } else if !record.warned && now >= end.saturating_sub(warn_before) {
                changed = true;
                record.warned = true;
                self.persist(&record);
                self.update_tracked(&record);
                self.event(
                    "lease_warned",
                    record.project.as_deref(),
                    record.service.as_deref(),
                    Some(&record.reference),
                    &json!({"port": record.port, "expires_at": end}),
                );
                effects.push(Effect::Notify(Notice::LeaseExpiring {
                    reference: record.reference.clone(),
                    project: record.project.clone(),
                    service: record.service.clone(),
                    port: record.port,
                    reason: record.reason.clone(),
                    expires_at: end,
                }));
            }
        }
        if changed {
            self.rebuild_views();
        }

        if now.saturating_sub(self.last_prune) >= PRUNE_EVERY {
            self.last_prune = now;
            let keep = i64::from(self.config.events_retention_days).saturating_mul(86_400);
            if let Err(error) = self.store.prune_events(now.saturating_sub(keep)) {
                self.problems.push(Problem::ActivityLogPrune {
                    detail: error.to_string(),
                });
            }
        }
        effects
    }

    pub(crate) fn update_tracked(&mut self, record: &InstanceRecord) {
        if let Some(slot) = self
            .tracked
            .iter_mut()
            .find(|candidate| candidate.id == record.id)
        {
            slot.clone_from(record);
        }
    }

    /// Stops the Temporal that owns `token`.
    ///
    /// # Errors
    ///
    /// `unknown_token` when no Temporal ever had this token.
    pub fn stop_by_token(&mut self, token: &str) -> Result<(StopResult, Vec<Effect>), ErrorBody> {
        let hash = token_hash(token);
        let live = self
            .tracked
            .iter()
            .find(|record| {
                record.token_hash.as_deref() == Some(hash.as_str()) && record.role == Role::Temporal
            })
            .cloned();
        let Some(mut record) = live else {
            // A token whose Temporal already ended is not an error: it is gone.
            return match self.store.instance_by_token_hash(&hash) {
                Ok(Some(ended)) => Ok((
                    StopResult {
                        instance_ref: ended.reference,
                        stopped: false,
                    },
                    Vec::new(),
                )),
                _ => Err(ErrorBody::new(
                    ErrorCode::UnknownToken,
                    "ese token no corresponde a ninguna prueba",
                )),
            };
        };
        let Some(target) = target_of(&record) else {
            // Reserved and never bound: there is nothing to stop yet; release it.
            record.state = State::Released;
            record.ended_at = Some(self.now());
            record.end_reason = Some("stopped_before_start".to_owned());
            self.persist(&record);
            self.update_tracked(&record);
            self.tracked.retain(|candidate| candidate.state.is_live());
            self.rebuild_views();
            return Ok((
                StopResult {
                    instance_ref: record.reference,
                    stopped: true,
                },
                Vec::new(),
            ));
        };
        record.state = State::Stopping;
        self.persist(&record);
        self.update_tracked(&record);
        self.rebuild_views();
        let reference = record.reference.clone();
        Ok((
            StopResult {
                instance_ref: reference.clone(),
                stopped: true,
            },
            vec![Effect::Terminate(TerminationRequest {
                reference,
                target,
                reason: StopReason::TokenRequest,
            })],
        ))
    }

    /// Adds time to the Temporal that owns `token`.
    ///
    /// # Errors
    ///
    /// `unknown_token` if there is no such running Temporal; `lease_limit` if the
    /// maximum total would be passed.
    pub fn extend_by_token(
        &mut self,
        token: &str,
        by_seconds: Option<u64>,
    ) -> Result<ExtendResult, ErrorBody> {
        let hash = token_hash(token);
        let reference = self
            .tracked
            .iter()
            .find(|record| {
                record.token_hash.as_deref() == Some(hash.as_str())
                    && record.role == Role::Temporal
                    && record.state == State::Running
            })
            .map(|record| record.reference.clone())
            .ok_or_else(|| {
                ErrorBody::new(
                    ErrorCode::UnknownToken,
                    "ese token no corresponde a ninguna prueba en marcha",
                )
            })?;
        self.extend_reference(&reference, by_seconds)
    }

    /// Adds time to the Temporal `reference` (also used by the interface).
    pub(crate) fn extend_reference(
        &mut self,
        reference: &str,
        by_seconds: Option<u64>,
    ) -> Result<ExtendResult, ErrorBody> {
        let by = by_seconds.unwrap_or(self.config.lease.extend_by.as_secs());
        let max = self.config.lease.max_total.as_secs();
        let mut record = self
            .tracked
            .iter()
            .find(|record| {
                record.reference == reference
                    && record.role == Role::Temporal
                    && record.state == State::Running
            })
            .cloned()
            .ok_or_else(|| ErrorBody::new(ErrorCode::NotFound, "esa prueba no está en marcha"))?;
        let total = record.leased_total_s.saturating_add(by);
        if total > max {
            return Err(ErrorBody::new(
                ErrorCode::LeaseLimit,
                format!(
                    "no se puede extender: pasaría del máximo de {} por prueba",
                    messages::span(self.config.lease.max_total)
                ),
            ));
        }
        record.leased_total_s = total;
        let end = record
            .expires_at
            .unwrap_or_else(|| self.now())
            .saturating_add(i64::try_from(by).unwrap_or(i64::MAX));
        record.expires_at = Some(end);
        record.warned = false;
        self.persist(&record);
        self.update_tracked(&record);
        self.event(
            "lease_extended",
            record.project.as_deref(),
            record.service.as_deref(),
            Some(&record.reference),
            &json!({"port": record.port, "expires_at": end, "by": by}),
        );
        self.rebuild_views();
        Ok(ExtendResult {
            expires_at: messages::iso(end),
            leased_total_seconds: total,
        })
    }

    /// Reports how stopping an instance ended, and settles its state.
    pub fn termination_finished(
        &mut self,
        reference: &str,
        reason: StopReason,
        outcome: &Terminated,
    ) {
        let now = self.now();
        let found = self
            .tracked
            .iter()
            .find(|record| record.reference == reference)
            .cloned();
        let view = self
            .views
            .iter()
            .find(|view| view.instance_ref == reference);
        let (project, service, port) = found.as_ref().map_or_else(
            || {
                (
                    view.and_then(|view| view.project.clone()),
                    view.and_then(|view| view.service.clone()),
                    view.and_then(|view| view.port),
                )
            },
            |record| (record.project.clone(), record.service.clone(), record.port),
        );
        let (kind, payload) = match outcome {
            Terminated::Stopped | Terminated::AlreadyGone => (
                if reason == StopReason::LeaseExpired {
                    "lease_expired"
                } else {
                    "stopped"
                },
                json!({"port": port}),
            ),
            Terminated::StillAlive => ("stop_failed", json!({"port": port, "why": "still_alive"})),
            Terminated::Refused(why) | Terminated::Failed(why) => {
                ("stop_failed", json!({"port": port, "why": why.to_string()}))
            }
        };
        if let Some(mut record) = found {
            match outcome {
                Terminated::Stopped | Terminated::AlreadyGone => {
                    record.state = State::Stopped;
                    record.ended_at = Some(now);
                    record.end_reason = Some(kind.to_owned());
                }
                // No retry loops: what could not be stopped is reported as forgotten.
                Terminated::StillAlive | Terminated::Refused(_) | Terminated::Failed(_) => {
                    record.state = State::ExpiredAlive;
                }
            }
            self.persist(&record);
            self.update_tracked(&record);
            self.tracked.retain(|candidate| candidate.state.is_live());
        }
        self.event(
            kind,
            project.as_deref(),
            service.as_deref(),
            Some(reference),
            &payload,
        );
        self.rebuild_views();
    }
}

/// What stopping a registered instance acts on.
pub(crate) fn target_of(record: &InstanceRecord) -> Option<Target> {
    match (&record.container_id, record.pid, record.pid_start) {
        (Some(id), _, _) => Some(Target::Container(id.clone())),
        (None, Some(pid), Some(start)) => Some(Target::Process(ProcessId {
            pid,
            start: ProcessStart(start),
        })),
        _ => None,
    }
}
