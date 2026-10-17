# Revisión de seguridad de la parada de procesos (T1.8)

> Fecha: 2026-10-17. Alcance: todo el código que termina procesos o contenedores y todo el que decide a quién se le aplica. Criterio (NF-06): **falsos positivos destructivos = 0**. Hecha a mano, línea por línea, contra `ARQUITECTURA.md` §8 y los ADR-006 y ADR-009.

## Qué puede terminar algo
| Camino | Quién lo puede pedir | Dónde se decide |
|---|---|---|
| Vencimiento de un Temporal | El reloj del núcleo | `engine/leases.rs::tick` |
| `devports stop <token>` | El CLI, con el token de **ese** Temporal | `engine/leases.rs::stop_by_token` |
| Detener desde el panel | Una persona, con un clic (y confirmación) | `engine/actions.rs::stop_instance`, comando de Tauri |
| Salir de la app | Una persona | no termina procesos ajenos |

No existe ningún otro camino. El socket no tiene operaciones que terminen Principales, procesos sin registrar ni contenedores (prueba `nothing_that_changes_files_or_stops_a_principal_exists_on_the_socket` y `the_cli_has_no_way_to_stop_or_extend_a_principal`).

## Hallazgos y resolución
| # | Hallazgo | Riesgo | Resolución |
|---|---|---|---|
| 1 | Entre listar el árbol y enviar la señal, un pid podía pasar a otro proceso. | Matar un proceso ajeno. | `signal_all` vuelve a leer `(pid, start)` justo antes de **cada** señal y no señala si cambió. Prueba `a_pid_handed_to_another_process_after_listing_is_not_signalled`. El resto es la ventana de microsegundos que ningún sistema sin `pidfd` cierra del todo; queda documentada. |
| 2 | Si la tabla de procesos no se podía leer, el resultado era «ya no existe» o «detenido». | Declarar detenido algo que sigue vivo (y no avisar). | Nuevo `Outcome::Unreadable` → `Terminated::Failed`. Nunca se informa «detenido» sin haberlo visto. Prueba `an_unreadable_process_table_is_never_reported_as_stopped`. |
| 3 | El nombre de proyecto o la ruta podían llevar a escribir un manifiesto en `~` o `/`. | Archivos en lugares inesperados (no destructivo, pero inapropiado). | `create_project` rechaza la raíz del volumen, `/Users` y el propio HOME. Prueba `a_folder_that_is_the_whole_home_or_the_volume_root_is_not_a_project`. |
| 4 | Los procesos de apps y del sistema aparecían como candidatos. | Ofrecer detener cosas que no son servidores. | No se leen ni se listan (`SYSTEM_PREFIXES`), y además la parada exige vínculo con una instancia registrada o una acción explícita de la persona. |

## Lo que se comprobó y está bien
- **Identidad:** el objetivo es `(pid, start_time)`; un pid reutilizado se deja en paz (`a_recycled_pid_is_not_the_target`, PT-03 con proptest).
- **Conjunto:** proceso + descendientes, con la guarda de «hijo más viejo que su padre» (padre reciclado). Nunca grupos (`kill_process`, no `kill_process_group`); la señal es por pid.
- **Prohibiciones absolutas:** el propio núcleo, sus ancestros, el pid 1 y procesos de otro uid se rechazan **antes** de enviar nada y rechazan el conjunto entero (`check_set`). `Refused` no envía ninguna señal (prueba `a_refused_target_receives_no_signal_at_all`).
- **Escalada:** `SIGTERM`, 5 s, `SIGKILL` solo a lo que sigue vivo, 2 s; lo que sobrevive queda como «olvidado» y no se reintenta en bucle.
- **Vínculo:** una instancia solo se ata a un proceso que empezó **después** de la reserva y escucha el puerto reservado; un extraño en ese puerto no se ata ni se detiene (I-05).
- **Contenedores:** solo `docker stop` (10 s). El tipo `Effect` no tiene forma de pedir `rm`; los contenedores `kind: data` no tienen vencimiento (D-02, D-04 reales con Docker).
- **Tokens:** 128 bits, solo se guarda el hash, nunca aparecen en `ls`, `log` ni en la base (U-08).
- **Persistencia:** un vencimiento pasado con el núcleo caído se ejecuta al volver (I-10), sobre procesos que se reverifican por `(pid, start)`.
- **Panel:** `open_url` solo abre `http` local; el contenido del panel es el empaquetado (CSP `default-src 'self'`).

## Riesgos aceptados
- La ventana residual entre la verificación y la señal (hallazgo 1).
- Un proceso que hace `exec` conserva `(pid, start)`: si cambia de programa entre dos lecturas, el vínculo ya era suyo; no se amplía el alcance de la parada.
- La caché de 30 s de `is_skipped` y de detalles puede retrasar medio minuto que un proceso que cambió de programa deje de ocultarse; no afecta a la parada.
