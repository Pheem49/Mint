use mint_mod_runtime::{preview, validate_package};
use serde_json::{Value, json};

fn manifest() -> Value {
    json!({"id":"meter","name":"Token meter","version":"1.0.0","sdkVersion":1,"entry":"main.ts","permissions":[]})
}
#[test]
fn session_and_viewport_getters_work_before_the_first_event() {
    let engine=mint_mod_runtime::Engine::new("export function register(mint){const u=mint.session.usage();const v=mint.ui.viewport();mint.ui.set('above_composer',{type:'text',text:u.context.tokens+' / '+v.columns});mint.on('ui_resize',()=>mint.log('resized'));}",&json!({}),|request| {
        let request:Value=serde_json::from_str(&request).unwrap();
        match request["kind"].as_str() { Some("usage")=>json!({"value":{"context":{"tokens":36100}}}).to_string(), Some("viewport")=>json!({"value":{"columns":80}}).to_string(), _=>panic!("Unexpected host request") }
    }).expect("Basic getters require no filesystem/tool capabilities");
    assert_eq!(
        engine.snapshot().unwrap()["ui"]["above_composer"]["text"],
        "36100 / 80"
    );
}
#[test]
fn preview_can_use_basic_getters_with_explicit_unavailable_values() {
    let value=preview(&manifest(),"export function register(mint){mint.ui.set('above_composer',{type:'text',text:String(mint.session.usage().context.tokens)+' / '+String(mint.ui.viewport().columns)});}").unwrap();
    assert_eq!(value["ui"]["above_composer"]["text"], "null / null");
}
#[test]
fn mod_test_runner_uses_real_events_fixtures_and_isolates_cases() {
    let source = "export function register(mint){mint.state.set('initial',mint.session.usage().context.tokens);mint.on('usage_update',e=>mint.ui.set('above_composer',{type:'text',text:String(e.context.tokens)}));}";
    let test_source = r#"export function registerTests(test){
        test('updates',async({emit,ui,equal,state})=>{equal(state('initial'),36100);await emit('usage_update',{context:{tokens:134400}});equal(ui('above_composer').text,'134400');},{usage:{context:{tokens:36100}}});
        test('isolated',({ui,equal})=>equal(ui('above_composer'),null));
        test('failure',({equal})=>equal(1,2,'intentional failure'));
    }"#;
    let result =
        mint_mod_runtime::run_tests(&manifest(), source, &[("weather.test.ts", test_source)])
            .unwrap();
    assert_eq!(result["passed"], 2);
    assert_eq!(result["failed"], 1);
    assert!(
        result["tests"][2]["error"]
            .as_str()
            .unwrap()
            .contains("intentional failure")
    );
}
#[test]
fn mod_test_runner_interrupts_a_loop_and_reports_missing_tests() {
    assert!(mint_mod_runtime::run_tests(&manifest(), "export function register(){}", &[]).is_err());
    let result = mint_mod_runtime::run_tests(
        &manifest(),
        "export function register(){}",
        &[(
            "loop.test.ts",
            "export function registerTests(test){test('loop',()=>{while(true){}});}",
        )],
    )
    .unwrap();
    assert_eq!(result["failed"], 1);
}
#[test]
fn mod_test_runner_rejects_invalid_ui_at_each_dispatch() {
    let source = "export function register(mint){mint.on('usage_update',()=>mint.ui.set('above_composer',{type:'html',html:'bad'}));mint.commands.register('repair',()=>mint.ui.set('above_composer',{type:'text',text:'Ready'}));}";
    let result=mint_mod_runtime::run_tests(&manifest(),source,&[("ui.test.ts","export function registerTests(test){test('invalid update',async({emit,command})=>{await emit('usage_update',{});await command('repair');});}")]).unwrap();
    assert_eq!(result["failed"], 1);
}
#[test]
fn mods_can_subscribe_to_context_compaction() {
    assert!(preview(&manifest(), "export function register(mint){mint.on('context_compacted', e => mint.log(e.contextStatus));}").is_ok());
}
#[test]
fn ui_rejects_unbounded_colors_and_invalid_row_directions() {
    assert!(preview(&manifest(), "export function register(mint){mint.ui.set('above_composer',{type:'text',text:'Weather',color:'url(evil)'});}").is_err());
    assert!(preview(&manifest(), "export function register(mint){mint.ui.set('above_composer',{type:'stack',direction:'diagonal',children:[]});}").is_err());
}
#[test]
fn weather_row_accepts_all_five_named_colors() {
    for color in ["yellow", "cyan", "blue", "magenta", "red"] {
        let source = format!(
            "export function register(mint){{mint.ui.set('above_composer',{{type:'stack',direction:'row',children:[{{type:'text',text:'Weather',color:'{color}'}}]}});}}"
        );
        assert!(
            preview(&manifest(), &source).is_ok(),
            "{color} must be supported"
        );
    }
}

#[test]
fn typescript_mod_registers_and_renders_without_node() {
    let source = r#"export function register(mint: any) {
      mint.ui.set('above_composer', {type:'text', text:'Usage unavailable'});
      mint.on('usage_update', (event: any) => mint.ui.set('above_composer', {type:'text', text:String(event.totalTokens)}));
    }"#;
    let result = preview(&manifest(), source).expect("valid mod loads");
    assert_eq!(result["ui"]["above_composer"]["text"], "Usage unavailable");
}

#[test]
fn invalid_ids_and_external_imports_are_rejected() {
    let mut m = manifest();
    m["id"] = json!("../outside");
    assert!(validate_package(&m, "export function register(mint) {}").is_err());
    assert!(
        validate_package(
            &manifest(),
            "import fs from 'node:fs'; export function register() {}"
        )
        .is_err()
    );
}

#[test]
fn infinite_loop_is_interrupted() {
    let start = std::time::Instant::now();
    assert!(
        preview(
            &manifest(),
            "export function register() { while (true) {} }"
        )
        .is_err()
    );
    assert!(start.elapsed() < std::time::Duration::from_secs(3));
}

#[test]
fn arbitrary_html_is_not_accepted_as_ui() {
    assert!(preview(&manifest(), "export function register(mint) { mint.ui.set('side_panel', {type:'html', html:'<script>bad</script>'}); }").is_err());
}

#[test]
fn exported_package_round_trips_only_shareable_source() {
    let bytes = mint_mod_runtime::pack(&manifest(), "export function register(mint) {}", "Example")
        .unwrap();
    let package = mint_mod_runtime::unpack(&bytes).unwrap();
    assert_eq!(package["manifest"]["id"], "meter");
    assert_eq!(package["source"], "export function register(mint) {}");
    assert_eq!(package["readme"], "Example");
    assert!(package.get("permissionsGranted").is_none());
}

#[test]
fn waiting_for_host_io_does_not_spend_callback_cpu_budget() {
    let code = "export function register(mint) { const text=mint.workspace.read('README.md'); for(let i=0;i<100000;i++){} mint.ui.set('side_panel',{type:'text',text}); }";
    let engine = mint_mod_runtime::Engine::new(code, &json!({}), |_| {
        std::thread::sleep(std::time::Duration::from_millis(350));
        json!({"value":"Read complete"}).to_string()
    })
    .expect("Host wait must not be counted as CPU execution");
    assert_eq!(
        engine.snapshot().unwrap()["ui"]["side_panel"]["text"],
        "Read complete"
    );
}

#[test]
fn async_registration_finishes_before_preview() {
    let value=preview(&manifest(),"export async function register(mint) { await Promise.resolve(); mint.ui.set('side_panel',{type:'text',text:'Ready'}); }").unwrap();
    assert_eq!(value["ui"]["side_panel"]["text"], "Ready");
}
