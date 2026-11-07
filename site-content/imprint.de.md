---
title: Impressum
description: Impressum und Anbieterangaben.
title_incomplete: Impressum (Platzhalter)
description_incomplete: Platzhalter für das Impressum. Angaben fehlen noch.
---
# Impressum

{{#if incomplete}}
> <strong>Platzhalter.</strong> Diese Seite enthält keine echten Angaben. Alle Felder mit TODO müssen vor der Veröffentlichung von der verantwortlichen Stelle ausgefüllt werden (Pflichtangaben nach § 5 DDG; Prüfung durch eine Fachperson empfohlen).
{{/if}}

## Anbieter

{{imprint.name|vollständiger Name bzw. Firma und Rechtsform}}
{{imprint.street|Straße und Hausnummer}}
{{imprint.city|PLZ und Ort}}

{{#if incomplete}}
{{#if publisher_name}}
Hinweis: Das README nennt die {{publisher_name}} als Betreuerin des Projekts. {{todo:prüfen, ob sie auch Anbieterin dieser Seite ist}}
{{/if}}
{{/if}}

{{#if incomplete|imprint.representative}}
## Vertretungsberechtigte

{{imprint.representative|Geschäftsführung}}
{{/if}}

## Kontakt

{{imprint.email|E-Mail-Adresse}}
{{imprint.phone|Telefon, falls vorgesehen}}

{{#if incomplete|imprint.register|imprint.vat_id}}
## Register und Steuer

{{imprint.register|Registergericht und Registernummer}}
{{imprint.vat_id|Umsatzsteuer-ID, falls vorhanden}}
{{/if}}

{{#if incomplete|imprint.content_responsible}}
## Verantwortlich für den Inhalt

{{imprint.content_responsible|Name und Anschrift}}
{{/if}}
