# Microsoft Teams Adapter

## Overview

Connects Clawdius to Microsoft Teams via the Bot Framework REST API.
Handles incoming messages from Teams channels and delivers responses
with OAuth token management.

## Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `TEAMS_APP_ID` | Yes | -- | Microsoft App ID from Azure |
| `TEAMS_APP_PASSWORD` | No | -- | Microsoft App Password from Azure |
| `TEAMS_SERVICE_URL` | No | `https://smba.trafficmanager.net/amer` | Bot Framework service URL |

The App ID is read from `api_token`. The App Password is read from
`settings.app_password`. The service URL is read from
`settings.service_url`.

## Setup

1. Go to the [Azure Portal](https://portal.azure.com) and sign in
2. Navigate to **Azure Active Directory > App registrations > New registration**
3. Give the app a name and register it
4. Under **Certificates & secrets**, create a new client secret
   and copy the value (this is the App Password)
5. Copy the **Application (client) ID** (this is the App ID)
6. Create a **Bot Channels Registration** in Azure and link it to
   the app registration
7. Configure the Teams channel in the Azure Portal
8. Configure:
   ```toml
   [gateways.teams]
   api_token = "your-microsoft-app-id"
   enabled = true

   [gateways.teams.settings]
   app_password = "your-client-secret"
   service_url = "https://smba.trafficmanager.net/amer"
   ```

## Features

- Text message sending and receiving
- Message editing for streaming responses
- OAuth access token caching (obtained automatically from the Bot Framework)
- Teams-specific channel data extraction
- Attachment download

## Limitations

- **Message editing is limited in Teams** -- edits are visible but Teams
  does not notify users of edits
- Requires Azure infrastructure for production use
- Message IDs are formatted as `service_url:conversation_id:activity_id`
  for edit operations
- Adaptive Card support is planned but not yet implemented
- Proactive messaging support is planned but not yet implemented

## Troubleshooting

**"TEAMS_APP_ID not set"**

Set `api_token` under `[gateways.teams]` to your Microsoft Application
(client) ID.

**"TEAMS_APP_PASSWORD not set"**

Add `app_password` to `[gateways.teams.settings]` with your client secret
value from Azure.

**OAuth token request fails**

Verify the App ID and App Password are correct. Ensure the app
registration has the `https://api.botframework.com/.default` scope
configured.

**Service URL errors**

The default service URL (`https://smba.trafficmanager.net/amer`) is for
the Americas region. Use `https://smba.trafficmanager.net/emea` for
Europe/Middle East/Africa or `https://smba.trafficmanager.net/apac` for
Asia-Pacific. You can also set a custom URL via `settings.service_url`.
