use devports_proto::views::{InstanceRole, InstanceState, InstanceView};

use super::*;
use crate::manifest;

fn process(cwd: &str, port: u16, argv: &[&str]) -> InstanceView {
    InstanceView {
        instance_ref: "r".to_owned(),
        project: None,
        service: None,
        role: InstanceRole::Unregistered,
        state: InstanceState::Healthy,
        kind: "app".to_owned(),
        port: Some(port),
        url: None,
        pid: Some(1),
        target_identity: Some("process-fingerprint".to_owned()),
        ports: vec![port],
        container: None,
        branch: None,
        worktree: None,
        cwd: Some(cwd.to_owned()),
        started_at: None,
        expires_at: None,
        reason: None,
        registered: false,
        launcher_chain: Vec::new(),
        duplicate_of: None,
        stale: None,
        shared: false,
        compose: None,
        argv: argv.iter().map(|part| (*part).to_owned()).collect(),
        image: None,
        ignored: false,
    }
}

fn container(name: &str, port: u16, image: &str, cwd: &str) -> InstanceView {
    InstanceView {
        container: Some(name.to_owned()),
        image: Some(image.to_owned()),
        ignored: false,
        argv: Vec::new(),
        pid: None,
        ..process(cwd, port, &[])
    }
}

// covers: I-15
#[test]
fn a_running_server_becomes_a_service_with_its_fingerprint() {
    let views = [process(
        "/work/shop/tienda-api",
        8080,
        &["/usr/bin/php", "spark", "serve", "--port", "8080"],
    )];
    let proposal = propose(Path::new("/work/shop"), &views);
    assert_eq!(proposal.project, "shop");
    let service = &proposal.services[0];
    assert_eq!(service.name, "tienda-api");
    assert_eq!(service.cwd.as_deref(), Some("./tienda-api"));
    assert_eq!(service.port, Some(8080));
    assert_eq!(
        service.start.as_deref(),
        Some("php spark serve --port ${PORT}")
    );
    assert_eq!(
        service.match_text.as_deref(),
        Some("php spark serve --port N")
    );
    assert!(!service.is_data);
    assert!(service.review.is_none());
}

#[test]
fn the_port_is_replaced_in_every_form_it_appears() {
    assert_eq!(
        start_command(
            &["node".into(), "vite".into(), "--port=5173".into()],
            5173,
            Path::new("/work/app")
        ),
        "node vite --port=${PORT}"
    );
    assert_eq!(
        start_command(
            &["php".into(), "-S".into(), "localhost:8000".into()],
            8000,
            Path::new("/work/app")
        ),
        "php -S localhost:${PORT}"
    );
    assert_eq!(
        start_command(
            &[
                "python3".into(),
                "-m".into(),
                "http.server".into(),
                "8001".into()
            ],
            8001,
            Path::new("/work/app")
        ),
        "python3 -m http.server ${PORT}"
    );
}

#[test]
fn a_launcher_is_flagged_for_review() {
    let views = [process("/work/shop/web", 5173, &["npm", "run", "dev"])];
    let proposal = propose(Path::new("/work/shop"), &views);
    let review = proposal.services[0].review.as_ref().unwrap().to_string();
    assert!(review.contains("npm"));
    assert!(proposal.to_yaml().contains("# revisar"));
}

#[test]
fn databases_and_caches_are_data_containers() {
    let views = [
        container("shop-redis", 6379, "redis:7", "/work/shop/infra"),
        container("shop-web", 8088, "nginx:latest", "/work/shop/infra"),
    ];
    let proposal = propose(Path::new("/work/shop"), &views);
    let redis = proposal
        .services
        .iter()
        .find(|s| s.name == "shop-redis")
        .unwrap();
    assert!(redis.is_data);
    assert_eq!(redis.docker.as_deref(), Some("shop-redis"));
    let web = proposal
        .services
        .iter()
        .find(|s| s.name == "shop-web")
        .unwrap();
    assert!(!web.is_data);
}

#[test]
fn things_outside_the_folder_or_already_in_a_service_are_left_out() {
    let mut owned = process("/work/shop/api", 8080, &["php"]);
    owned.service = Some("api".to_owned());
    let views = [owned, process("/elsewhere/other", 9000, &["node"])];
    assert!(propose(Path::new("/work/shop"), &views).services.is_empty());
}

#[test]
fn two_services_with_the_same_name_get_their_port_appended() {
    let views = [
        process("/work/shop/a/api", 8080, &["php"]),
        process("/work/shop/b/api", 8081, &["php"]),
    ];
    let names: Vec<String> = propose(Path::new("/work/shop"), &views)
        .services
        .into_iter()
        .map(|service| service.name)
        .collect();
    assert_eq!(names, ["api-8080", "api-8081"]);
}

// covers: I-15
#[test]
fn the_proposed_yaml_is_a_valid_manifest() {
    let views = [
        process(
            "/work/shop/tienda-api",
            8080,
            &["/usr/bin/php", "spark", "serve", "--port", "8080"],
        ),
        process("/work/shop/web", 5173, &["npm", "run", "dev"]),
        container("shop-redis", 6379, "redis:7", "/work/shop/infra"),
    ];
    let yaml = propose(Path::new("/work/shop"), &views).to_yaml();
    let loaded = manifest::parse_checked(&yaml, Path::new("proposal.yml"), None)
        .unwrap_or_else(|error| panic!("the proposal must load: {error}\n{yaml}"));
    assert_eq!(loaded.manifest.services.len(), 3);
    assert_eq!(
        loaded.manifest.services["shop-redis"].kind,
        crate::manifest::ServiceKind::Data
    );
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
}

#[test]
fn an_empty_proposal_is_still_valid() {
    let yaml = propose(Path::new("/work/empty"), &[]).to_yaml();
    assert!(
        manifest::parse_checked(&yaml, Path::new("p.yml"), None).is_ok(),
        "{yaml}"
    );
}

#[test]
fn slugs_are_valid_names() {
    assert_eq!(slug("Tienda API"), "tienda-api");
    assert_eq!(slug("--x--"), "x");
    assert_eq!(slug("!!!"), "servicio");
    assert!(manifest::is_valid_name(&slug("Ñandú café 2")));
}

#[test]
fn awkward_commands_are_quoted_in_yaml() {
    assert_eq!(scalar("php spark serve"), "php spark serve");
    assert_eq!(scalar("a: b"), "\"a: b\"");
    assert_eq!(scalar("--flag"), "\"--flag\"");
    assert_eq!(scalar("say \"hi\""), "say \"hi\"");
    assert_eq!(scalar("x #y"), "\"x #y\"");
}

// covers: I-15, RF-05
#[test]
fn paths_in_a_start_command_are_tidied_so_they_mean_the_same_later() {
    let cwd = Path::new("/work/app");
    let command = |argv: &[&str], port| {
        start_command(
            &argv
                .iter()
                .map(|part| (*part).to_owned())
                .collect::<Vec<_>>(),
            port,
            cwd,
        )
    };
    // `..` collapses and a file inside the folder becomes relative.
    assert_eq!(
        command(
            &[
                "node",
                "/work/app/node_modules/.bin/../.pnpm/vite@8/node_modules/vite/bin/vite.js",
                "--port",
                "5173"
            ],
            5173
        ),
        "node node_modules/.pnpm/vite@8/node_modules/vite/bin/vite.js --port ${PORT}"
    );
    // The folder itself is `.`, a path outside it stays absolute, options are left alone.
    assert_eq!(
        command(&["php", "-S", "127.0.0.1:8192", "-t", "/work/app"], 8192),
        "php -S 127.0.0.1:${PORT} -t ."
    );
    assert_eq!(
        command(&["python3", "/elsewhere/../tools/serve.py", "9000"], 9000),
        "python3 /tools/serve.py ${PORT}"
    );
    assert_eq!(
        command(&["ruby", "app.rb", "--bind=0.0.0.0"], 4567),
        "ruby app.rb --bind=0.0.0.0"
    );
}

// covers: UXR-3.1
#[test]
fn proposed_commands_keep_observed_arguments_literal_in_the_shell() {
    let command = start_command(
        &[
            "node".to_owned(),
            "a path/server.js".to_owned(),
            "$(touch /tmp/not-executed)".to_owned(),
            "8080".to_owned(),
        ],
        8080,
        Path::new("/work"),
    );
    assert_eq!(
        command,
        "node 'a path/server.js' '$(touch /tmp/not-executed)' ${PORT}"
    );
    let mut service = process_service(
        Path::new("/work"),
        Path::new("/work"),
        &process("/work", 8080, &["node"]),
    );
    service.name = "true".to_owned();
    service.start = Some("false".to_owned());
    let proposal = Proposal {
        project: "true".to_owned(),
        root: PathBuf::from("/work"),
        services: vec![service],
    };
    let loaded = crate::manifest::parse(&proposal.to_yaml(), Path::new("m.yml")).unwrap();
    assert_eq!(loaded.project, "true");
    assert_eq!(loaded.services["true"].start.as_deref(), Some("false"));
}
