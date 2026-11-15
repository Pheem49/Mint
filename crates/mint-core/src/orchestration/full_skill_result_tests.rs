use super::*;

#[test]
fn large_loaded_skill_results_reach_the_model_without_losing_middle_instructions() {
    let content = format!(
        "[Loaded skill: review]\n{}\nREQUIRED-MIDDLE-INSTRUCTION\n{}",
        "a".repeat(40_000),
        "z".repeat(40_000)
    );
    assert_eq!(model_tool_result(None, "read_file", &content), content);
    assert!(model_tool_result(None, "run_shell", &"x".repeat(80_000)).len() < 20_000);
}
