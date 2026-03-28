//! Tool-level access control for remote relay access.
//!
//! Controls which tools are exposed to remote MCP clients through the relay.
//! This is separate from the per-host tool filtering in `ServerConfig` —
//! relay ACL is an additional layer that restricts what's reachable remotely.

use serde::{Deserialize, Serialize};

/// Access control rules for a tunnel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclRules {
    /// If Some, only these tools are allowed remotely.
    /// If None, all tools from the gateway are exposed.
    pub allowed_tools: Option<Vec<String>>,
}

impl AclRules {
    /// Allow all tools remotely.
    pub fn allow_all() -> Self {
        Self {
            allowed_tools: None,
        }
    }

    /// Only allow specific tools remotely.
    pub fn allow_only(tools: Vec<String>) -> Self {
        Self {
            allowed_tools: Some(tools),
        }
    }

    /// Check if a tool is allowed for remote access.
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        match &self.allowed_tools {
            None => true,
            Some(tools) => tools.iter().any(|t| t == tool_name),
        }
    }

    /// Check if a method is allowed (tools/list is always allowed).
    pub fn is_method_allowed(&self, method: &str, tool_name: Option<&str>) -> bool {
        match method {
            // Always allow listing and initialization
            "tools/list" | "initialize" | "notifications/initialized" => true,
            // For tool calls, check the specific tool
            "tools/call" => match tool_name {
                Some(name) => self.is_tool_allowed(name),
                None => false, // tools/call without a tool name is invalid
            },
            // Block unknown methods by default
            _ => false,
        }
    }
}

impl Default for AclRules {
    fn default() -> Self {
        Self::allow_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_all() {
        let acl = AclRules::allow_all();
        assert!(acl.is_tool_allowed("any_tool"));
        assert!(acl.is_tool_allowed("another_tool"));
    }

    #[test]
    fn test_allow_only() {
        let acl = AclRules::allow_only(vec!["get_issues".to_string(), "search".to_string()]);
        assert!(acl.is_tool_allowed("get_issues"));
        assert!(acl.is_tool_allowed("search"));
        assert!(!acl.is_tool_allowed("delete_repo"));
    }

    #[test]
    fn test_method_allowed() {
        let acl = AclRules::allow_only(vec!["get_issues".to_string()]);

        // tools/list always allowed
        assert!(acl.is_method_allowed("tools/list", None));

        // tools/call checks the tool
        assert!(acl.is_method_allowed("tools/call", Some("get_issues")));
        assert!(!acl.is_method_allowed("tools/call", Some("delete_repo")));

        // tools/call without tool name is rejected
        assert!(!acl.is_method_allowed("tools/call", None));

        // Unknown methods rejected
        assert!(!acl.is_method_allowed("resources/list", None));
    }

    #[test]
    fn test_default_is_allow_all() {
        let acl = AclRules::default();
        assert!(acl.allowed_tools.is_none());
        assert!(acl.is_tool_allowed("any_tool"));
    }

    #[test]
    fn test_allow_only_empty_list() {
        let acl = AclRules::allow_only(vec![]);
        assert!(!acl.is_tool_allowed("any_tool"));
    }

    #[test]
    fn test_allow_only_single_tool() {
        let acl = AclRules::allow_only(vec!["only_this".to_string()]);
        assert!(acl.is_tool_allowed("only_this"));
        assert!(!acl.is_tool_allowed("other"));
    }

    #[test]
    fn test_initialize_method_always_allowed() {
        let acl = AclRules::allow_only(vec![]); // empty allowlist
        assert!(acl.is_method_allowed("initialize", None));
    }

    #[test]
    fn test_notifications_initialized_always_allowed() {
        let acl = AclRules::allow_only(vec![]);
        assert!(acl.is_method_allowed("notifications/initialized", None));
    }

    #[test]
    fn test_tools_list_always_allowed() {
        let acl = AclRules::allow_only(vec![]);
        assert!(acl.is_method_allowed("tools/list", None));
    }

    #[test]
    fn test_acl_rules_clone() {
        let acl = AclRules::allow_only(vec!["tool1".to_string(), "tool2".to_string()]);
        let cloned = acl.clone();
        assert_eq!(cloned.allowed_tools, acl.allowed_tools);
    }

    #[test]
    fn test_acl_rules_serialization() {
        let acl = AclRules::allow_only(vec!["get_data".to_string()]);
        let json = serde_json::to_string(&acl).unwrap();
        assert!(json.contains("get_data"));

        let deserialized: AclRules = serde_json::from_str(&json).unwrap();
        assert!(deserialized.is_tool_allowed("get_data"));
        assert!(!deserialized.is_tool_allowed("delete_data"));
    }

    #[test]
    fn test_allow_all_serialization() {
        let acl = AclRules::allow_all();
        let json = serde_json::to_string(&acl).unwrap();
        // allowed_tools should be null
        assert!(json.contains("null"));

        let deserialized: AclRules = serde_json::from_str(&json).unwrap();
        assert!(deserialized.allowed_tools.is_none());
    }

    #[test]
    fn test_tool_name_case_sensitive() {
        let acl = AclRules::allow_only(vec!["get_issues".to_string()]);
        assert!(acl.is_tool_allowed("get_issues"));
        assert!(!acl.is_tool_allowed("GET_ISSUES"));
        assert!(!acl.is_tool_allowed("Get_Issues"));
    }

    #[test]
    fn test_method_with_tool_allowed() {
        let acl = AclRules::allow_only(vec!["get".to_string(), "search".to_string()]);
        assert!(acl.is_method_allowed("tools/call", Some("get")));
        assert!(acl.is_method_allowed("tools/call", Some("search")));
        assert!(!acl.is_method_allowed("tools/call", Some("delete")));
    }
}
