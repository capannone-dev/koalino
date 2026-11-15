<!-- This directory holds the web dashboard's translations and the code that picks and applies a language. -->

# Translations (Issue #124)

The dashboard speaks the same ten languages as the app: English, Italian, Spanish, French,
German, Portuguese (Brazil), Simplified Chinese, Japanese, Russian and Arabic (right to left).

## Contents

- `messages/<lang>.json`: the messages, one file per language. `en.json` defines every key. A
  value is plain text with `{name}` placeholders, or an object of plural forms: `=0`, `=1`, `=2`,
  or an `Intl.PluralRules` category (`one`, `few`, `many`, `other`), chosen by the `count` value.
- `index.ts`: the language list, `matchLanguage` (the owner's cookie, else the browser's
  `Accept-Language`, else English), `textDirection`, and `translator`, which builds a `Translate`
  for a language. `englishT` is the default.
- `server.ts`: `requestLanguage` and `getT` for server components. They read the request's
  `koalino-language` cookie and `Accept-Language` header.
- `client.tsx`: `I18nProvider`, which the root layout wraps around every page, plus the `useT` and
  `useLanguage` hooks for client components.

## How it fits

The root layout resolves the language on the server. It sets `<html lang dir>` and passes the
language to `I18nProvider`, so server and browser render the same text and hydration matches.
Without a provider (as in tests) everything is English. Plain helpers in `lib/` that word text
take a `t: Translate` argument, which defaults to English. Components pass their own `t`. There is
no global language, so concurrent requests cannot mix languages. Settings → *Language*
(`components/language-settings.tsx`) stores the choice in the cookie and refreshes the page.

## Adding or changing a message

1. Add the key to `messages/en.json`, then to every other `messages/<lang>.json`, keeping the
   placeholder names.
2. Use `t("key", { name: value })` (`useT()` in a client component, `await getT()` in a server one).
3. `tests/i18n.test.tsx` fails if a language lacks a key, changes a placeholder, or lacks an
   `other` plural form.

The translations were machine-made, so native speakers should review them. Kept in English:
- brand names;
- developer error messages that are not shown to the owner;
- environment variable names.
