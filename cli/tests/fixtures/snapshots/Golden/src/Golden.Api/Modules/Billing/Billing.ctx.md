# billing

What the Billing module is for, in the business's words: the capability it owns and who relies on it.

## Boundaries

- **Inside**: the data and rules Billing owns and is the only module allowed to write.
- **Outside**: what it leaves to other modules. It references them by id, never through an EF relationship.

## Design notes

### Wiring
`BillingModule` registers the module's services and maps its routes; the module registry calls both. Record
each decision a stranger would otherwise undo here, with its reason, as the module grows.
