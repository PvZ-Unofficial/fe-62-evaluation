use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("backend crate must stay below crates/backends/pvz_portable")
        .to_path_buf()
}

fn read(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        .replace("\r\n", "\n")
}

fn bridge_source(root: &Path) -> String {
    let directory = root.join("crates/backends/pvz_portable/pvzp-rs/cpp");
    let mut files: Vec<_> = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| matches!(path.extension().and_then(|ext| ext.to_str()), Some("cpp" | "inc")))
        .collect();
    files.sort();
    files.into_iter().map(read).collect::<Vec<_>>().join("\n")
}

fn allowed_bridge_copy(function: &str, line: &str) -> bool {
    if line.contains("sizeof(float)")
        && [
            "row_pick_weight_bits",
            "row_pick_last_picked_bits",
            "row_pick_second_last_picked_bits",
        ]
        .iter()
        .any(|name| line.starts_with(&format!("PVZP_RS_DEFINE_WORLD_ROW_VALUE({name},")))
    {
        return true;
    }
    // Text output and explicit single-slot relocation do not materialize pool traversal.
    match function {
        "pvzp_rs_world_caption_text" => line == "std::memcpy(output, label, size); *length = size; return Ok();",
        "pvzp_rs_world_relocate_plant_slot" => matches!(
            line,
            "std::memcpy(temporary, source, sizeof(Plant));"
                | "std::memcpy(source, destination, sizeof(Plant));"
                | "std::memcpy(destination, temporary, sizeof(Plant));"
                | "std::memcpy(destination, source, sizeof(Plant));"
        ),
        _ => false,
    }
}

#[test]
fn wrapper_dependency_points_only_downward() {
    let root = workspace_root();
    let wrapper = read(root.join("crates/backends/pvz_portable/pvzp-rs/Cargo.toml"));
    for forbidden in [
        "rsvz-pvz-portable-backend",
        "rsvz-runtime",
        "rsvz-current",
        "rsvz-pvz-portable-tooling",
        "rsvz =",
    ] {
        assert!(!wrapper.contains(forbidden), "pvzp-rs depends upward on {forbidden}");
    }
    let backend = read(root.join("crates/backends/pvz_portable/backend/Cargo.toml"));
    assert!(backend.contains("pvzp-rs.workspace = true"));
}

#[test]
fn object_pool_paths_do_not_materialize_collections() {
    let root = workspace_root();
    for relative in [
        "crates/backends/pvz_portable/pvzp-rs/src/world.rs",
        "crates/backends/pvz_portable/pvzp-rs/src/entity.rs",
        "crates/backends/pvz_portable/backend/src/handles.rs",
    ] {
        let source = read(root.join(relative));
        for forbidden in ["Vec<", "vec![", ".collect(", "copy_nonoverlapping", "memcpy"] {
            assert!(
                !source.contains(forbidden),
                "{relative} materializes pool traversal with {forbidden}"
            );
        }
    }

    let bridge = read(root.join("../PvZ-Portable/src/PvzpLib/NativeControls.cpp")) + &bridge_source(&root);
    for forbidden in [
        "CopyPlants",
        "CopyZombies",
        "GetAllPlants",
        "GetAllZombies",
        "SnapshotBuffer",
    ] {
        assert!(
            !bridge.contains(forbidden),
            "bridge contains bulk object path {forbidden}"
        );
    }
    let mut function = "";
    for line in bridge.lines() {
        if let Some(signature) = line.strip_prefix("extern \"C\" ") {
            function = signature.split('(').next().unwrap().split_whitespace().last().unwrap();
        } else if line == "}" {
            function = "";
        }
        if line.contains("memcpy") {
            assert!(
                allowed_bridge_copy(function, line.trim()),
                "unexpected bridge bulk copy: {line}"
            );
        }
    }
}

#[test]
fn bridge_copy_exceptions_remain_scoped_and_single_object() {
    let caption = "std::memcpy(output, label, size); *length = size; return Ok();";
    let relocation = "std::memcpy(destination, source, sizeof(Plant));";
    assert!(allowed_bridge_copy("pvzp_rs_world_caption_text", caption));
    assert!(allowed_bridge_copy("pvzp_rs_world_relocate_plant_slot", relocation));
    assert!(!allowed_bridge_copy("pvzp_rs_plant_pool_get", caption));
    assert!(!allowed_bridge_copy("pvzp_rs_plant_pool_get", relocation));
    assert!(!allowed_bridge_copy(
        "pvzp_rs_world_relocate_plant_slot",
        "std::memcpy(destination, source, count * sizeof(Plant));"
    ));
}

#[test]
fn portable_dispatch_precedes_each_native_logic_update() {
    let root = workspace_root();
    let source = read(root.join("../PvZ-Portable/src/LawnApp.cpp"));
    let start = source.find("void LawnApp::UpdateFrames()").expect("UpdateFrames");
    let body = &source[start
        ..source[start..]
            .find("\n}\n")
            .map_or(source.len(), |end| start + end + 3)];
    let dispatch = body.find("PvzpNative::BeforeUpdate()").expect("Portable dispatch");
    let counter = body.find("mAppCounter++").expect("native app counter");
    let begin = body.find("PvzpNative::BeginLogicFrame()").expect("event begin");
    let update = body.find("SexyApp::UpdateFrames()").expect("native update");
    let end = body.find("PvzpNative::EndLogicFrame()").expect("event end");
    assert!(dispatch < counter && counter < begin && begin < update && update < end);
    assert!(body.find("PvzpNative::RunBattleFastForward()").unwrap() > end);
    let native = read(root.join("../PvZ-Portable/src/PvzpLib/NativeControls.cpp"));
    let counts = native
        .split("int UpdateCount(")
        .nth(1)
        .unwrap()
        .split("bool ContinueUpdateBatch")
        .next()
        .unwrap();
    assert!(
        !counts.contains("gFastForward"),
        "battle skipping must not expand normal updates"
    );
    let fast = native
        .split("void RunBattleFastForward()")
        .nth(1)
        .unwrap()
        .split("int FastForwardPerformance()")
        .next()
        .unwrap();
    let mut position = 0;
    for operation in [
        "BeforeUpdate()",
        "BeginLogicFrame()",
        "++gLawnApp->mAppCounter",
        "mBoard->ProcessDeleteQueue()",
        "mBoard->Update()",
        "mEffectSystem->ProcessDeleteQueue()",
        "CheckForGameEnd()",
        "EndLogicFrame()",
    ] {
        position += fast[position..].find(operation).expect(operation) + operation.len();
    }
    assert!(!fast.contains("SexyApp::UpdateFrames()"));
}

#[test]
fn portable_seed_chooser_readiness_is_classified_in_shared_game_logic() {
    let root = workspace_root();
    let bridge = read(root.join("../PvZ-Portable/src/PvzpLib/NativeControls.cpp")) + &bridge_source(&root);
    assert!(!bridge.contains("SeedChooserOpeningReady"));
    assert!(!bridge.contains("SeedChooserCardActionsReady"));

    let backend = read(root.join("crates/backends/pvz_portable/backend/src/runtime.rs"));
    assert!(backend.contains("classify_seed_chooser_readiness("));
    let host = read(root.join("crates/backends/pvz_portable/backend/src/host/readiness.rs"));
    assert!(host.contains("readiness.opening_action()"));
}

#[test]
fn modifier_queries_do_not_call_back_into_rust() {
    let root = workspace_root();
    let source = read(root.join("../PvZ-Portable/src/PvzpLib/NativeControls.cpp"));
    let start = source
        .find("#define PVZP_DEFINE_RULE_QUERY")
        .expect("modifier query block");
    let end = source[start..]
        .find("int UpdateCount")
        .expect("end of modifier query block")
        + start;
    assert!(!source[start..end].contains("rsvz_pvzp_"));
}

#[test]
fn portable_scripts_are_runtime_plugins_not_game_link_inputs() {
    let root = workspace_root();
    let tooling = read(root.join("crates/backends/pvz_portable/tooling/src/lib.rs"));
    assert!(tooling.contains("crate-type = [\\\"cdylib\\\"]"));
    assert!(!tooling.contains("staticlib"));

    let run_cli = read(root.join("crates/backends/pvz_portable/tooling/src/run_cli.rs"));
    assert!(run_cli.contains("PVZP_PLUGIN"));
    assert!(!run_cli.contains("RSVZ_PORTABLE_STATICLIB"));

    let cmake = read(root.join("../PvZ-Portable/CMakeLists.txt"));
    assert!(!cmake.contains("RSVZ"));
    let loader = read(root.join("../PvZ-Portable/src/PvzpLib/Plugin.cpp"));
    assert!(loader.contains("LoadLibraryW"));
    assert!(loader.contains("FreeLibrary"));
}

#[test]
fn native_wave_random_scopes_stay_at_their_execution_boundaries() {
    let root = workspace_root();
    let board = read(root.join("../PvZ-Portable/src/Lawn/Board.cpp"));
    for hook in ["BeginSurvivalWaveInit", "BeginWaveSpawn", "BeginRowPick"] {
        assert!(
            board.contains(&format!("PvzpNative::{hook}(")),
            "missing native random boundary: {hook}"
        );
    }
}
