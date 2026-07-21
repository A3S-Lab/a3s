# Connector integration

A3S Web can expose the [OOMOL connector catalog](https://oomol.com/zh-cn/apps/) to
new Code agents through the standard MCP configuration path. The integration
supports OOMOL's hosted connector and self-hosted OpenConnector deployments.
In either mode, provider credentials remain behind the connector boundary; A3S
receives tool schemas, safe connection identity, execution results, and the
single connector credential needed to reach that boundary.

## Hosted OOMOL

1. Connect the required services in the
   [OOMOL Console](https://console.oomol.com/connections).
2. Create an API key on the
   [OOMOL API keys page](https://console.oomol.com/api-key).
3. In A3S Web, open **Settings → Integrations → Connector**.
4. Select **Connect hosted OOMOL**, enter the API key, and save the category.
5. Restart A3S Code Web, as indicated by the category effect badge.

The generated MCP server is equivalent to this configuration:

```text
name: oomol-connector
transport: streamable-http
url: https://connector.oomol.com/mcp
Authorization: <raw OOMOL API key>
```

The hosted service expects the API key itself as the `Authorization` header
value. A3S must not prepend `Bearer` in this mode.

## Self-hosted OpenConnector

Deploy OpenConnector by following OOMOL's
[self-hosting guide](https://oomol.com/zh-cn/docs/openconnector-self-hosting/),
connect the required providers, and create a runtime token on its Access page.
Then choose **Connect self-hosted**, enter the complete `/mcp` endpoint and the
runtime token, save, and restart A3S Code Web.

The default local configuration is equivalent to:

```text
name: oomol-connector
transport: streamable-http
url: http://localhost:3000/mcp
Authorization: Bearer <runtime token>
```

The token is optional only when authentication is disabled on the self-hosted
runtime. Production deployments should protect the runtime endpoint, encrypt
stored provider credentials, and restrict the allowed Actions at the
OpenConnector boundary.

## Agent tools

OpenConnector exposes a compact discovery-oriented MCP surface. The current
public contract includes:

- `list_apps`
- `list_connections`
- `search_actions`
- `get_action_guide`
- `execute_action`

Agents discover an Action before executing it instead of loading thousands of
provider operations into every prompt. Named OpenConnector connections remain
selectable through the tool inputs.

## Secret handling and lifecycle

The Web configuration API returns stored authorization values only as the
`[configured]` marker. Saving an unchanged marker preserves the existing local
secret; replacing or clearing it is explicit. Switching between hosted and
self-hosted mode clears a masked credential because the two deployments use
different authorization formats and the browser never receives the original
value to transform.

The connector is persisted as a normal `mcp_servers "oomol-connector"` entry in
the A3S ACL configuration. A3S omits this managed entry from the generic MCP list so a
newly entered key cannot also appear in a plain generic header field; other MCP
servers retain the full advanced editor. Configuration changes take effect
after the local A3S Code Web service restarts.
