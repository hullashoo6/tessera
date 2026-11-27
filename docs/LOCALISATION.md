# Localisation: not planned

**Owner decision, 27 Nov 2026 19:49 IST: Tessera stays English only.** Do
not build translations, a language setting or translation tooling unless
the owner asks again. (Earlier the same evening Hindi-first was proposed;
the owner then decided against additional languages.)

Already language-neutral, no work needed: dates, numbers and sizes follow
the system's regional format (`Intl`).

Notes kept in case the owner ever changes this: about 450 interface texts
in `apps/desktop/src` and about 140 sentences from the backend (Smart Scan
labels and reasons, error messages). The approach would be English text as
the key (`t('...')` with `{name}` placeholders), catalogues per language,
plurals through `Intl.PluralRules`, and each language reviewed by a fluent
speaker before it is offered.
