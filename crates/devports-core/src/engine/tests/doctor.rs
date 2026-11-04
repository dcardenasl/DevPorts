//! `doctor`: the reason a process is, or is not, a declared service (ADR-044).

use devports_proto::doctor::{DoctorArgs, DoctorMismatch, DoctorVerdict};

use super::*;

fn verdict(world: &World, port: u16) -> DoctorVerdict {
    let result = world.engine.doctor(&DoctorArgs {
        port: Some(port),
        pid: None,
    });
    assert_eq!(result.processes.len(), 1, "one process listens on {port}");
    result.processes[0].verdict.clone()
}

// covers: ROB-5.2
#[test]
fn doctor_lists_the_search_settings_and_the_projects_without_asking_about_a_process() {
    let world = world();
    let result = world.engine.doctor(&DoctorArgs::default());
    assert!(result.processes.is_empty());
    assert_eq!(result.projects.len(), 1);
    assert_eq!(result.projects[0].name, "shop");
    assert_eq!(result.projects[0].services, ["admin", "api", "cache"]);
    assert!(result.max_depth > 0);
}

// covers: ROB-5.2
#[test]
fn doctor_says_which_service_a_process_is() {
    let mut world = world();
    let now = epoch(&world) - 100;
    world.scan(vec![proc(
        900,
        1,
        now,
        "/work/shop/api",
        Some(8080),
        &["php", "spark", "serve"],
    )]);
    assert_eq!(
        verdict(&world, 8080),
        DoctorVerdict::Matched {
            project: "shop".to_owned(),
            service: "api".to_owned(),
        }
    );
}

// covers: ROB-5.2
#[test]
fn doctor_shows_the_command_it_expected_and_the_one_it_saw() {
    let mut world = world();
    let now = epoch(&world) - 100;
    world.scan(vec![proc(
        901,
        1,
        now,
        "/work/shop/api",
        Some(8080),
        &["node", "server.js", "--port", "8080"],
    )]);
    let DoctorVerdict::NoService {
        project,
        candidates,
        port_of,
    } = verdict(&world, 8080)
    else {
        panic!("expected a project without a matching service");
    };
    assert_eq!(project, "shop");
    // The process sits in `api`'s folder with another command; the port is `api`'s.
    assert_eq!(port_of.as_deref(), Some("api"));
    let api = candidates
        .iter()
        .find(|candidate| candidate.service == "api")
        .expect("api is a candidate");
    assert_eq!(
        api.reason,
        DoctorMismatch::CommandDiffers {
            expected: "php spark serve".to_owned(),
            observed: "node server.js --port N".to_owned(),
        }
    );
    let admin = candidates
        .iter()
        .find(|candidate| candidate.service == "admin")
        .expect("admin is a candidate");
    assert!(matches!(admin.reason, DoctorMismatch::FolderOutside { .. }));
    let cache = candidates
        .iter()
        .find(|candidate| candidate.service == "cache")
        .expect("cache is a candidate");
    assert_eq!(cache.reason, DoctorMismatch::ContainerService);
}

// covers: ROB-5.2
#[test]
fn doctor_tells_a_process_outside_every_project_from_one_without_a_folder() {
    let mut world = world();
    let now = epoch(&world) - 100;
    let nowhere = proc(
        902,
        1,
        now,
        "/tmp/elsewhere",
        Some(9100),
        &["python3", "-m", "x"],
    );
    let mut unknown = proc(
        903,
        1,
        now,
        "/tmp/other",
        Some(9101),
        &["python3", "-m", "y"],
    );
    unknown.info.cwd = None;
    world.scan(vec![nowhere, unknown]);
    assert_eq!(verdict(&world, 9100), DoctorVerdict::OutsideProjects);
    assert_eq!(verdict(&world, 9101), DoctorVerdict::NoFolder);
}

// covers: ROB-5.2
#[test]
fn doctor_finds_a_process_by_pid_as_well() {
    let mut world = world();
    let now = epoch(&world) - 100;
    world.scan(vec![proc(
        904,
        1,
        now,
        "/work/shop/api",
        Some(8080),
        &["php", "spark", "serve"],
    )]);
    let result = world.engine.doctor(&DoctorArgs {
        port: None,
        pid: Some(904),
    });
    assert_eq!(result.processes.len(), 1);
    assert_eq!(result.processes[0].fingerprint, "php spark serve");
}
