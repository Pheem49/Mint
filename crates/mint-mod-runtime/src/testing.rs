use super::*;
use std::sync::Mutex;

impl Engine {
    fn load_tests(&self, compiled: &str) -> Result<Vec<Value>, String> {
        *self.deadline.lock().unwrap() = cpu_time::ThreadTime::now().as_duration();
        let metadata = self
            .context
            .with(|ctx| -> rquickjs::Result<String> {
                ctx.eval::<(), _>(include_str!("test_sdk.js"))?;
                let module = Module::declare(ctx.clone(), "tests", compiled)?;
                let (module, promise) = module.eval()?;
                promise.finish::<()>()?;
                let register: Function = module.get("registerTests")?;
                let test: Function = ctx.eval("__test")?;
                let returned: rquickjs::Value = register.call((test,))?;
                if let Some(promise) = returned.as_promise() {
                    promise.clone().finish::<()>()?;
                }
                ctx.eval("JSON.stringify(__tests.map(t=>({name:t.name,fixtures:t.fixtures})))")
            })
            .map_err(|e| self.error(e))?;
        serde_json::from_str(&metadata).map_err(|e| e.to_string())
    }
    fn run_test(&self, index: usize) -> Result<(), String> {
        *self.deadline.lock().unwrap() = cpu_time::ThreadTime::now().as_duration();
        self.context
            .with(|ctx| -> rquickjs::Result<()> {
                let promise: Promise = ctx.eval(format!("__runTest({index})"))?;
                promise.finish::<()>()
            })
            .map_err(|e| self.error(e))?;
        self.snapshot()?;
        Ok(())
    }
}
pub fn run_tests(manifest: &Value, source: &str, files: &[(&str, &str)]) -> Result<Value, String> {
    if files.is_empty() || files.len() > 32 {
        return Err("Provide 1–32 .test.ts/.test.js files".into());
    }
    let manifest: Manifest = serde_json::from_value(manifest.clone()).map_err(|e| e.to_string())?;
    let compiled = compile(&manifest, source)?;
    let mut results = vec![];
    for (filename, test_source) in files {
        let test_code = compile(&manifest, test_source)?;
        let registry = Engine::new("export function register(){}", &json!({}), |_| {
            json!({"error":"No host access while registering tests"}).to_string()
        })?;
        let tests = registry.load_tests(&test_code)?;
        if tests.is_empty() {
            return Err(format!("No tests registered in {filename}"));
        }
        for (index, test) in tests.iter().enumerate() {
            let fixtures = test["fixtures"].clone();
            let usage = Arc::new(Mutex::new(
                fixtures
                    .get("usage")
                    .cloned()
                    .unwrap_or_else(unavailable_usage),
            ));
            let current = usage.clone();
            let permissions = manifest.permissions.clone();
            let result = (|| -> Result<(), String> {
                let engine = Engine::new(
                    &compiled,
                    &fixtures.get("state").cloned().unwrap_or_else(|| json!({})),
                    move |request| {
                        let request: Value = serde_json::from_str(&request).unwrap_or_default();
                        match request["kind"].as_str() {
                        Some("usage")=>json!({"value":current.lock().unwrap().clone()}),
                        Some("viewport")=>json!({"value":fixtures.get("viewport").cloned().unwrap_or_else(unavailable_viewport)}),
                        Some("test_usage")=>{*current.lock().unwrap()=request["value"].clone();json!({"value":null})},
                        Some("test_snapshot")=>match validate_snapshot(&request["value"]){Ok(())=>json!({"value":null}),Err(error)=>json!({"error":error})},
                        Some("read") if permissions.iter().any(|p|p=="workspace.read")=>{
                            match request["path"].as_str().and_then(|path|fixtures["files"].get(path)).filter(|v|v.is_string()) {
                                Some(value)=>json!({"value":value}),None=>json!({"error":"No file fixture for this path"}),
                            }
                        }
                        _=>json!({"error":"Test host capability unavailable; external tools and I/O are disabled"}),
                    }.to_string()
                    },
                )?;
                let registered = engine.load_tests(&test_code)?;
                if registered.get(index).and_then(|t| t["name"].as_str()) != test["name"].as_str() {
                    return Err("Test registration must be deterministic".into());
                }
                engine.run_test(index)
            })();
            results.push(json!({"file":filename,"name":test["name"],"passed":result.is_ok(),"error":result.err()}));
        }
    }
    let passed = results.iter().filter(|t| t["passed"] == true).count();
    Ok(json!({"passed":passed,"failed":results.len()-passed,"tests":results}))
}
