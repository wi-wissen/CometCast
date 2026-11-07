---
title: Imprint
description: Imprint and provider details.
title_incomplete: Imprint (placeholder)
description_incomplete: Placeholder for the legal imprint. Details are missing.
---
# Imprint

{{#if incomplete}}
> <strong>Placeholder.</strong> This page holds no real data. Every field marked TODO must be filled in by the responsible party before publication (mandatory details under German law, section 5 DDG; review by a professional recommended). The German version of this page is the one that counts.
{{/if}}

## Provider

{{imprint.name|full name or company and legal form}}
{{imprint.street|street and number}}
{{imprint.city|postcode and city}}

{{#if incomplete}}
{{#if publisher_name}}
Note: the README names {{publisher_name}} as steward of the project. {{todo:check whether it is also the provider of this site}}
{{/if}}
{{/if}}

{{#if incomplete|imprint.representative}}
## Represented by

{{imprint.representative|managing director(s)}}
{{/if}}

## Contact

{{imprint.email|email address}}
{{imprint.phone|phone, if intended}}

{{#if incomplete|imprint.register|imprint.vat_id}}
## Register and tax

{{imprint.register|register court and number}}
{{imprint.vat_id|VAT ID, if any}}
{{/if}}

{{#if incomplete|imprint.content_responsible}}
## Responsible for content

{{imprint.content_responsible|name and address}}
{{/if}}
