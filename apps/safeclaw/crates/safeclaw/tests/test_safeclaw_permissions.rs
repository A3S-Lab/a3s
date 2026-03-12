/// Integration test for SafeClaw with A3S Code v1.3.4
///
/// Tests the new permission wildcard matching feature in SafeClaw context.
///
/// Run with: cargo test --test test_safeclaw_permissions
// Import from the dependency, not the crate name
use a3s_code::permissions::{PermissionDecision, PermissionPolicy};

#[tokio::test]
async fn test_safeclaw_permission_wildcard() {
    // Test 1: Verify wildcard matching works in SafeClaw context
    let policy = PermissionPolicy::permissive()
        .deny("mcp__longvt__*")
        .deny("bash");

    // MCP longvt tools should be denied
    assert_eq!(
        policy.check("mcp__longvt__search", &serde_json::json!({})),
        PermissionDecision::Deny,
        "mcp__longvt__search should be denied"
    );

    assert_eq!(
        policy.check("mcp__longvt__create_memory", &serde_json::json!({})),
        PermissionDecision::Deny,
        "mcp__longvt__create_memory should be denied"
    );

    // bash should be denied
    assert_eq!(
        policy.check("bash", &serde_json::json!({"command": "ls"})),
        PermissionDecision::Deny,
        "bash should be denied"
    );

    // Other tools should be allowed
    assert_eq!(
        policy.check("mcp__pencil__draw", &serde_json::json!({})),
        PermissionDecision::Allow,
        "mcp__pencil__draw should be allowed"
    );

    assert_eq!(
        policy.check("read", &serde_json::json!({"file_path": "test.txt"})),
        PermissionDecision::Allow,
        "read should be allowed"
    );

    println!("✅ SafeClaw permission wildcard test passed");
}

#[tokio::test]
async fn test_safeclaw_agent_permissions() {
    // Test 2: Verify agent service can use new permission features
    // This is a smoke test to ensure SafeClaw's agent module
    // is compatible with the updated a3s-code v1.3.4

    // Create a permission policy with wildcard deny
    let policy = PermissionPolicy::permissive().deny("mcp__*"); // Deny all MCP tools

    // Verify the policy works
    assert_eq!(
        policy.check("mcp__any__tool", &serde_json::json!({})),
        PermissionDecision::Deny
    );

    assert_eq!(
        policy.check("read", &serde_json::json!({"file_path": "test.txt"})),
        PermissionDecision::Allow
    );

    println!("✅ SafeClaw agent permissions test passed");
}

#[test]
fn test_version_compatibility() {
    // Test 3: Verify we're using the correct version
    // This test ensures SafeClaw is compiled against a3s-code v1.3.4+

    let policy = PermissionPolicy::permissive().deny("mcp__test__*");

    // This should work with v1.3.4+ (wildcard matching)
    assert_eq!(
        policy.check("mcp__test__foo", &serde_json::json!({})),
        PermissionDecision::Deny,
        "Wildcard matching should work (requires v1.3.4+)"
    );

    println!("✅ Version compatibility test passed");
}
