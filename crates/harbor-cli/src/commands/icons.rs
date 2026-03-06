//! MCP icon data URIs for Harbor and known providers.
//!
//! Icons are embedded as base64 data URIs (PNG or SVG) so they work
//! over stdio transports without requiring the client to fetch URLs.

/// Harbor logo — 128x128 PNG (cargo ship).
pub const HARBOR_ICON: &str = "data:image/png;base64,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";

// Provider icons as URL-encoded SVG data URIs.
// Each is a 48x48 SVG with a colored circle and white text/symbol.

/// Google — multicolor "G" on white circle
pub const ICON_GOOGLE: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' rx='10' fill='%234285F4'/%3E%3Ctext x='24' y='33' font-family='Arial,sans-serif' font-size='28' font-weight='bold' fill='white' text-anchor='middle'%3EG%3C/text%3E%3C/svg%3E";

/// Atlassian / Jira — blue "A" icon
pub const ICON_ATLASSIAN: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' rx='10' fill='%230052CC'/%3E%3Ctext x='24' y='33' font-family='Arial,sans-serif' font-size='28' font-weight='bold' fill='white' text-anchor='middle'%3EA%3C/text%3E%3C/svg%3E";

/// Slack — purple "S" icon
pub const ICON_SLACK: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' rx='10' fill='%234A154B'/%3E%3Ctext x='24' y='33' font-family='Arial,sans-serif' font-size='28' font-weight='bold' fill='white' text-anchor='middle'%3ES%3C/text%3E%3C/svg%3E";

/// Notion — black "N" icon
pub const ICON_NOTION: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' rx='10' fill='%23000000'/%3E%3Ctext x='24' y='33' font-family='Arial,sans-serif' font-size='28' font-weight='bold' fill='white' text-anchor='middle'%3EN%3C/text%3E%3C/svg%3E";

/// GitHub — dark "GH" icon
pub const ICON_GITHUB: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' rx='10' fill='%23181717'/%3E%3Ctext x='24' y='32' font-family='Arial,sans-serif' font-size='20' font-weight='bold' fill='white' text-anchor='middle'%3EGH%3C/text%3E%3C/svg%3E";

/// Stripe — purple "S" icon
pub const ICON_STRIPE: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' rx='10' fill='%23635BFF'/%3E%3Ctext x='24' y='33' font-family='Arial,sans-serif' font-size='28' font-weight='bold' fill='white' text-anchor='middle'%3ES%3C/text%3E%3C/svg%3E";

/// Cloudflare — orange "CF" icon
pub const ICON_CLOUDFLARE: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' rx='10' fill='%23F6821F'/%3E%3Ctext x='24' y='32' font-family='Arial,sans-serif' font-size='20' font-weight='bold' fill='white' text-anchor='middle'%3ECF%3C/text%3E%3C/svg%3E";

/// Vercel — black triangle icon
pub const ICON_VERCEL: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' rx='10' fill='%23000000'/%3E%3Cpolygon points='24,10 38,38 10,38' fill='white'/%3E%3C/svg%3E";

/// Brave — orange "B" icon
pub const ICON_BRAVE: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' rx='10' fill='%23FB542B'/%3E%3Ctext x='24' y='33' font-family='Arial,sans-serif' font-size='28' font-weight='bold' fill='white' text-anchor='middle'%3EB%3C/text%3E%3C/svg%3E";

/// Gmail — red "M" icon
pub const ICON_GMAIL: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' rx='10' fill='%23EA4335'/%3E%3Ctext x='24' y='33' font-family='Arial,sans-serif' font-size='28' font-weight='bold' fill='white' text-anchor='middle'%3EM%3C/text%3E%3C/svg%3E";

/// Google Calendar — blue "C" icon
pub const ICON_GCAL: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' rx='10' fill='%234285F4'/%3E%3Ctext x='24' y='33' font-family='Arial,sans-serif' font-size='28' font-weight='bold' fill='white' text-anchor='middle'%3EC%3C/text%3E%3C/svg%3E";

/// Google Drive — green/blue "D" icon
pub const ICON_DRIVE: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' rx='10' fill='%230F9D58'/%3E%3Ctext x='24' y='33' font-family='Arial,sans-serif' font-size='28' font-weight='bold' fill='white' text-anchor='middle'%3ED%3C/text%3E%3C/svg%3E";

/// Google Sheets — green "S" icon
pub const ICON_SHEETS: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' rx='10' fill='%230F9D58'/%3E%3Ctext x='24' y='33' font-family='Arial,sans-serif' font-size='28' font-weight='bold' fill='white' text-anchor='middle'%3ES%3C/text%3E%3C/svg%3E";

/// Google Docs — blue "D" icon
pub const ICON_DOCS: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' rx='10' fill='%234285F4'/%3E%3Ctext x='24' y='33' font-family='Arial,sans-serif' font-size='28' font-weight='bold' fill='white' text-anchor='middle'%3ED%3C/text%3E%3C/svg%3E";

/// Match a tool name to a provider icon URI.
/// Returns `None` if no provider pattern matches (caller should fall back to Harbor icon).
pub fn icon_for_tool(tool_name: &str) -> Option<&'static str> {
    let name = tool_name.to_lowercase();

    // Google product-specific matches (before generic google)
    if name.starts_with("gmail") || name.contains("gmail") {
        return Some(ICON_GMAIL);
    }
    if name.starts_with("gcal") || name.contains("calendar") {
        return Some(ICON_GCAL);
    }
    if name.starts_with("drive_") || name.contains("drive") {
        return Some(ICON_DRIVE);
    }
    if name.starts_with("sheets_") || name.contains("spreadsheet") {
        return Some(ICON_SHEETS);
    }
    if name.starts_with("docs_") || name.contains("docs_documents") {
        return Some(ICON_DOCS);
    }
    if name.starts_with("slides_") {
        return Some(ICON_GOOGLE);
    }
    if name.starts_with("forms_") {
        return Some(ICON_GOOGLE);
    }
    if name.starts_with("meet_") {
        return Some(ICON_GOOGLE);
    }
    if name.starts_with("keep_") {
        return Some(ICON_GOOGLE);
    }
    if name.starts_with("classroom_") {
        return Some(ICON_GOOGLE);
    }
    if name.starts_with("chat_") {
        return Some(ICON_GOOGLE);
    }
    if name.starts_with("people_") || name.starts_with("tasks_") {
        return Some(ICON_GOOGLE);
    }

    // Atlassian / Jira / Confluence
    if name.contains("jira") || name.contains("atlassian") || name.contains("confluence") {
        return Some(ICON_ATLASSIAN);
    }

    // Slack
    if name.starts_with("slack_") || name.contains("slack") {
        return Some(ICON_SLACK);
    }

    // Notion
    if name.starts_with("notion") {
        return Some(ICON_NOTION);
    }

    // Stripe
    if name.contains("stripe")
        || name.contains("coupon")
        || name.contains("invoice")
        || name.contains("payment_intent")
        || name.contains("subscription")
        || name.starts_with("retrieve_balance")
        || name.starts_with("list_customers")
        || name.starts_with("list_disputes")
        || name.starts_with("list_prices")
        || name.starts_with("list_products")
    {
        return Some(ICON_STRIPE);
    }

    // Cloudflare (Workers, R2, KV, D1, Hyperdrive)
    if name.contains("cloudflare")
        || name.starts_with("workers_")
        || name.starts_with("r2_")
        || name.starts_with("kv_")
        || name.starts_with("d1_")
        || name.starts_with("hyperdrive_")
        || name.contains("migrate_pages_to_workers")
        || name.contains("search_cloudflare")
    {
        return Some(ICON_CLOUDFLARE);
    }

    // Vercel
    if name.contains("vercel")
        || name.starts_with("get_deployment")
        || name.starts_with("list_deployments")
        || name.starts_with("get_project")
        || name.starts_with("list_projects")
        || name.starts_with("list_teams")
        || name.starts_with("get_runtime_logs")
        || name.starts_with("deploy_to_vercel")
    {
        return Some(ICON_VERCEL);
    }

    // Brave Search
    if name.starts_with("brave_") {
        return Some(ICON_BRAVE);
    }

    // GitHub
    if name.contains("github") {
        return Some(ICON_GITHUB);
    }

    // Generic Google (catch-all for remaining google workspace tools)
    if name.contains("google")
        || name.starts_with("admin_")
        || name.starts_with("accounts_")
        || name.starts_with("set_active_account")
        || name.starts_with("modelarmor_")
        || name.starts_with("workspaceevents_")
    {
        return Some(ICON_GOOGLE);
    }

    None
}

/// Build an MCP `icons` array value for a given tool name.
pub fn tool_icons_value(tool_name: &str) -> Option<serde_json::Value> {
    let icon_uri = icon_for_tool(tool_name)?;
    Some(serde_json::json!([
        {
            "src": icon_uri,
            "sizes": ["48x48"]
        }
    ]))
}

/// Build the MCP `icons` array for the Harbor server itself.
pub fn harbor_server_icons() -> serde_json::Value {
    serde_json::json!([
        {
            "src": HARBOR_ICON,
            "mimeType": "image/png",
            "sizes": ["128x128"]
        }
    ])
}
