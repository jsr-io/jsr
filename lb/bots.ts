// Copyright 2024 the JSR authors. All rights reserved. MIT license.

interface BotPattern {
  header: string;
  pattern: RegExp;
}

const BOT_PATTERNS: BotPattern[] = [
  // Googlebot detection via "from" header
  { header: "from", pattern: /^googlebot\(at\)googlebot\.com/i },
  // Slack bots
  { header: "user-agent", pattern: /^Slack/i },
  // Iframely (used by Notion for link previews)
  { header: "user-agent", pattern: /^Iframely/i },
  // Twitter bot
  { header: "user-agent", pattern: /^Twitter/i },
  // WhatsApp
  { header: "user-agent", pattern: /^WhatsApp/i },
  // Discord bot
  { header: "user-agent", pattern: /^Mozilla\/5\.0 \(compatible; Discordbot/i },
];

export function isBot(request: Request): boolean {
  for (const { header, pattern } of BOT_PATTERNS) {
    const value = request.headers.get(header);
    if (value && pattern.test(value)) {
      return true;
    }
  }
  return false;
}
