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
}
