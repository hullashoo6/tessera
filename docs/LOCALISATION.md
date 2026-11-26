# Localisation plan (not started)

Roadmap Phase 2 asks for at least Hindi, Spanish, German, French, Portuguese
and Japanese (DissectMac offers seven languages). Measured on 27 Nov 2026:
about 450 interface texts in `apps/desktop/src` and about 140 sentences sent
by the backend (Smart Scan labels and reasons, error messages).

## Owner decisions needed first

1. Which languages, in which order (suggestion: Spanish, German, French,
   Portuguese (Brazil), Japanese, Hindi).
2. Who reviews translations. Machine-quality text in a paid app hurts more
   than English does; each language should be read by a fluent speaker before
   it is offered.

## Approach

* English text is the key: `t('Scan drive')`, with `{name}` placeholders
  (`t('Scanned in {time}', { time })`). Catalogues in
  `apps/desktop/src/i18n/<code>.ts`; a missing entry falls back to English.
* Plurals through `Intl.PluralRules` (Hindi and Japanese differ from English);
  `plural()` in `format.ts` takes both English forms as keys.
* Settings > Language: System (default) or a listed language. Only languages
  whose catalogue is complete are listed; a Vitest test extracts every `t()`
  key from the source and fails if a listed language lacks one.
* Backend text: Smart Scan labels and reasons become fixed strings (the one
  reason that includes a browser name becomes one string per browser) and are
  translated by the interface with the same catalogues. Other backend errors
  stay English at first, then move to error codes.
* Dates, numbers and sizes already follow the system locale (`Intl`).
* Right-to-left languages are out of scope for the first round.
* Website: a second phase (Astro supports per-language routes).

## Effort

One session for the framework and moving all English text into `t()` (the
English interface must look identical; screen tests prove it), then about an
hour per language plus review.
