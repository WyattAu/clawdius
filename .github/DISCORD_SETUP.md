# Discord Server Setup Guide

This guide explains how to set up the official Clawdius Discord server.

---

## Server Structure

### Categories & Channels

```
📁 WELCOME
├── #welcome - Welcome message and rules
├── #announcements - Official announcements
├── #code-of-conduct - Community guidelines
└── #roles - Self-assignable roles

📁 GENERAL
├── #general - General discussion
├── #introductions - Introduce yourself
└── #off-topic - Non-Clawdius discussion

📁 HELP
├── #getting-started - New user questions
├── #configuration - Config and setup help
├── #troubleshooting - Bug reports and issues
└── #provider-support - LLM provider questions

📁 DEVELOPMENT
├── #development - Core development discussion
├── #plugins - Plugin development
├── #architecture - System design discussions
└── #security - Security-related topics

📁 SHOWCASE
├── #show-your-work - Share your projects
├── #workflows - Interesting Clawdius workflows
└── #plugins-showcase - Share your plugins

📁 ENTERPRISE
├── #enterprise-general - Enterprise discussion
├── #sso-integration - SSO setup help
└── #compliance - Compliance questions

📁 COMMUNITY
├── #contributing - How to contribute
├── #documentation - Docs improvements
├── #translations - i18n efforts
└── #events - Community events
```

---

## Roles

### Bot Roles
| Role | Color | Permissions |
|------|-------|-------------|
| `Clawdius Bot` | Purple | Manage roles, embed links |

### Staff Roles
| Role | Color | Permissions |
|------|-------|-------------|
| `Admin` | Red | All permissions |
| `Moderator` | Orange | Kick, ban, manage messages |
| `Core Team` | Gold | Manage channels |
| `Contributor` | Green | Special access |

### User Roles
| Role | Color | How to Get |
|------|-------|------------|
| `Verified` | Blue | Accept rules |
| `Enterprise User` | Purple | Enterprise license |
| `Plugin Developer` | Green | Published plugin |
| `Translator` | Teal | Contributed translations |

---

## Bots & Integrations

### Required Bots

1. **Carl-bot** - Logging, moderation, reaction roles
   - Invite: `https://carl.gg`
   - Features: Auto-role, logging, moderation

2. **GitHub** - Issue/PR notifications
   - Setup: Settings → Integrations → GitHub
   - Channels: #announcements, #development

3. **Dyno** - Backup moderation
   - Invite: `https://dyno.gg`
   - Features: Auto-mod, announcements

### Custom Clawdius Bot (Optional)

```python
# discord_bot.py
import discord
from discord.ext import commands

intents = discord.Intents.default()
intents.message_content = True

bot = commands.Bot(command_prefix='!', intents=intents)

@bot.command()
async def version(ctx):
    """Show Clawdius version"""
    await ctx.send("🦀 Clawdius v1.0.0-rc.1")

@bot.command()
async def docs(ctx):
    """Link to documentation"""
    await ctx.send("📚 https://docs.clawdius.dev")

@bot.command()
async def install(ctx, platform: str = "cargo"):
    """Show installation instructions"""
    platforms = {
        "cargo": "`cargo install clawdius`",
        "nix": "`nix shell github:clawdius/clawdius`",
        "source": "```bash\ngit clone https://github.com/clawdius/clawdius\ncd clawdius\ncargo build --release\n```"
    }
    await ctx.send(platforms.get(platform, "Unknown platform. Use: cargo, nix, source"))

bot.run("YOUR_BOT_TOKEN")
```

---

## Welcome Message

```
# Welcome to the Clawdius Community! 🦀

Clawdius is the high-assurance AI coding assistant built with Rust.

## Quick Links
- 📚 Documentation: https://docs.clawdius.dev
- 💻 GitHub: https://github.com/clawdius/clawdius
- 🐛 Issues: https://github.com/clawdius/clawdius/issues
- 💬 Discussions: https://github.com/clawdius/clawdius/discussions

## Getting Started
1. Read the rules in #code-of-conduct
2. Get roles in #roles
3. Introduce yourself in #introductions
4. Ask questions in #getting-started

## Need Help?
- New to Clawdius? Check #getting-started
- Configuration issues? Try #configuration
- Found a bug? Report in #troubleshooting

**"Build like an Emperor. Protect like a Sentinel."**
```

---

## Moderation Settings

### Auto-Moderation Rules

| Rule | Action | Threshold |
|------|--------|-----------|
| Spam | Mute | 5 messages in 5 seconds |
| Links | Delete | New users (< 1 day) |
| Mentions | Warn | > 5 mentions |
| Caps | Delete | > 70% caps |

### Slow Mode

| Channel | Slow Mode |
|---------|-----------|
| #announcements | Off |
| #general | 5s |
| #getting-started | 10s |
| #development | 5s |

---

## Verification System

### Step 1: Rules Agreement
Users must react to rules message to get `Verified` role.

### Step 2: Onboarding (Optional)
Simple quiz about Clawdius basics.

### Step 3: Role Assignment
Users can self-assign roles in #roles:
- 🖥️ Linux user
- 🍎 macOS user
- 🪟 Windows user
- 🦀 Rust developer
- 🐍 Python developer
- 🟨 JavaScript developer

---

## Analytics & Metrics

### Weekly Report
- New members
- Active users
- Most active channels
- Top contributors

### Monthly Report
- Member growth
- Message volume
- Support ticket resolution time

---

## Launch Checklist

- [ ] Create server with categories above
- [ ] Add all roles
- [ ] Configure Carl-bot
- [ ] Set up GitHub integration
- [ ] Create welcome message
- [ ] Configure auto-mod rules
- [ ] Test verification system
- [ ] Invite initial community members
- [ ] Announce in GitHub Discussions
- [ ] Add invite link to README.md

---

## Invite Link Settings

```
https://discord.gg/clawdius

Settings:
- Max uses: Unlimited
- Max age: Never
- Grant role: @Verified (auto)
```

---

## Contact

For questions about Discord setup:
- GitHub: @clawdius
- Email: team@clawdius.dev
