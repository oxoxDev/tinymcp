//! The curated first-party servers.
//!
//! Every entry but Slack and Swiggy was checked against what the official
//! registry publishes for that exact name: the hosted endpoint, its transport,
//! and the headers it declares. Slack and Swiggy publish no registry entry;
//! their endpoints and client rules come from each vendor's own developer
//! documentation. An entry here is
//! a claim made to the user, so extend the list only from a vendor's own
//! publication.

use super::types::{CuratedAuth, CuratedServer, CuratedTransport};

/// Canonical first-party servers, with how each is reached.
pub const CURATED_SERVERS: &[CuratedServer] = &[
    CuratedServer {
        qualified_name: "io.github.github/github-mcp-server",
        display_name: "GitHub",
        description: "Connect AI assistants to GitHub - manage repos, issues, PRs, and workflows \
                      through natural language.",
        remote_url: "https://api.githubcopilot.com/mcp/",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Token,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "com.notion/mcp",
        display_name: "Notion",
        description: "Official Notion MCP server",
        remote_url: "https://mcp.notion.com/mcp",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Oauth,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "com.stripe/mcp",
        display_name: "Stripe",
        description: "MCP server integrating with Stripe - tools for customers, products, \
                      payments, and more.",
        remote_url: "https://mcp.stripe.com",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Oauth,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "com.atlassian/atlassian-mcp-server",
        display_name: "Atlassian Rovo MCP Server",
        description: "Connect to Atlassian Jira, Confluence, Loom, and more to search, create, \
                      and manage your work.",
        remote_url: "https://mcp.atlassian.com/v2/mcp",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Oauth,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "app.linear/linear",
        display_name: "Linear",
        description: "MCP server for Linear project management and issue tracking",
        remote_url: "https://mcp.linear.app/mcp",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Oauth,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "com.gitlab/mcp",
        display_name: "GitLab",
        description: "Official GitLab MCP Server",
        remote_url: "https://gitlab.com/api/v4/mcp",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Oauth,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "com.paypal.mcp/mcp",
        display_name: "PayPal",
        description: "PayPal MCP server provides access to PayPal services and operations for \
                      AI assistants",
        remote_url: "https://mcp.paypal.com/mcp",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Token,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "com.cloudflare.mcp/mcp",
        display_name: "Cloudflare",
        description: "Cloudflare MCP servers",
        remote_url: "https://docs.mcp.cloudflare.com/mcp",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::None,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "com.airtable/mcp",
        display_name: "Airtable",
        description: "Official Airtable MCP server — database and operations layer for agents.",
        remote_url: "https://mcp.airtable.com/mcp",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Oauth,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "com.supabase/mcp",
        display_name: "Supabase",
        description: "MCP server for interacting with the Supabase platform",
        remote_url: "https://mcp.supabase.com/mcp",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Oauth,
        icon_url: Some("https://supabase.com/favicon/favicon-196x196.png"),
    },
    CuratedServer {
        qualified_name: "com.vercel/vercel-mcp",
        display_name: "Vercel",
        description: "An MCP server for Vercel",
        remote_url: "https://mcp.vercel.com",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Oauth,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "com.webflow/mcp",
        display_name: "Webflow",
        description: "AI-powered design and management for Webflow Sites",
        remote_url: "https://mcp.webflow.com/mcp",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Oauth,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "com.wix/mcp",
        display_name: "Wix",
        description: "A Model Context Protocol server for Wix AI tools",
        remote_url: "https://mcp.wix.com/mcp",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Oauth,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "com.slack/mcp",
        display_name: "Slack",
        description: "Official Slack MCP server for searching and acting on workspace messages, \
                      channels, and users",
        remote_url: "https://mcp.slack.com/mcp",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::OauthPreregistered,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "com.swiggy/food",
        display_name: "Swiggy Food",
        description: "Official Swiggy MCP server for restaurant discovery, menus, food ordering \
                      and order tracking",
        remote_url: "https://mcp.swiggy.com/food",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Oauth,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "com.swiggy/instamart",
        display_name: "Swiggy Instamart",
        description: "Official Swiggy MCP server for Instamart quick-commerce grocery shopping",
        remote_url: "https://mcp.swiggy.com/im",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Oauth,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "com.swiggy/dineout",
        display_name: "Swiggy Dineout",
        description: "Official Swiggy MCP server for restaurant table reservations",
        remote_url: "https://mcp.swiggy.com/dineout",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Oauth,
        icon_url: None,
    },
    CuratedServer {
        qualified_name: "com.swiggy/scenes",
        display_name: "Swiggy Scenes",
        description: "Official Swiggy MCP server for discovering events and shows and booking \
                      tickets",
        remote_url: "https://mcp.swiggy.com/scenes",
        transport: CuratedTransport::StreamableHttp,
        auth: CuratedAuth::Oauth,
        icon_url: None,
    },
];
