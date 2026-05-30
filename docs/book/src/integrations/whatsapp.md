# WhatsApp Adapter

## Overview

Connects Clawdius to WhatsApp via the Meta Cloud API (WhatsApp Business
Platform). Handles text message sending and receiving through webhook-based
incoming messages and the Graph API for outgoing messages.

## Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `WHATSAPP_ACCESS_TOKEN` | Yes | -- | Permanent access token from Meta Developer Portal |
| `WHATSAPP_PHONE_NUMBER_ID` | Yes | -- | Phone number ID for sending messages |
| `WHATSAPP_VERIFY_TOKEN` | No | -- | Token for webhook verification (HMAC) |

The access token is read from `api_token`. The phone number ID is read
from `settings.phone_number_id`. The verify token is read from
`webhook_secret`.

## Setup

1. Create a Meta Developer account at
   [developers.facebook.com](https://developers.facebook.com)
2. Create a new app and add the **WhatsApp** product
3. Add a phone number to the WhatsApp Business Platform
4. Copy the **Permanent Access Token** and **Phone Number ID**
5. Configure the webhook callback URL in the Meta Developer Portal
   pointing to your Clawdius gateway
6. Configure:
   ```toml
   [gateways.whatsapp]
   api_token = "EAAxxxxxxxx"
   webhook_secret = "your-verify-token"
   enabled = true

   [gateways.whatsapp.settings]
   phone_number_id = "1234567890"
   ```

## Features

- Text message sending and receiving
- Webhook-based incoming message processing
- Webhook verification challenge handling
- Contact profile extraction (wa_id, name)
- Attachment download via Media API
- Media message type detection in metadata

## Limitations

- **WhatsApp does NOT support message editing.** When the gateway attempts
  to edit a message, the adapter sends a new message with a `(corrected) `
  prefix instead.
- **24-hour response window** -- WhatsApp requires responses within 24
  hours of the user's message. After this window closes, only pre-approved
  message templates can be sent.
- Rate limited by Meta's API tier
- Interactive messages are planned but not yet implemented
- Message template support is planned but not yet implemented

## Troubleshooting

**"WHATSAPP_ACCESS_TOKEN not set"**

Set `api_token` under `[gateways.whatsapp]` to your permanent access
token from the Meta Developer Portal.

**"WHATSAPP_PHONE_NUMBER_ID not set"**

Add `phone_number_id` to `[gateways.whatsapp.settings]`.

**Webhook verification fails**

Ensure the `webhook_secret` matches the verify token configured in the
Meta Developer Portal. The webhook mode must be `subscribe`.

**"send failed with 401"**

The access token has expired or is invalid. Generate a new permanent
token in the Meta Developer Portal.

**Messages not received**

Verify the webhook callback URL in the Meta Developer Portal is pointing
to the correct Clawdius endpoint and that the gateway is publicly
accessible.
