# JD-307: keep pasted media embedded

2026-11-06. This is an object-transfer resource fix, not complete Office clipboard support.

A destination resource with an external link and identical cached bytes could be reused by PasteObjects, turning an embedded source image into a linked destination image. The new regression failed before the fix: the pasted image reused the linked resource ID. The lower-level Presentation::add_media also deduplicated by bytes alone, ignoring both link status and content type.

Both lookup layers now reuse only embedded resources with matching content type and bytes. Repeated pastes still deduplicate the correct embedded resource; the linked resource and a same-byte/different-type resource remain intact. The regression checks two pastes, resource identity and bytes, link absence, unchanged destination links, undo back to the exact initial model and redo. Its bytes and unavailable file URL are synthetic; no URL is opened or external resource fetched.

Model/engine/OPC tests passed (144 tests), as did strict library Clippy. Adapted source fingerprints were updated for the two imported files while preserving original source fingerprints.

Rich text still cannot transfer image-resource bytes: cross-document text with unresolved picture formatting is conservatively rejected, while copying a supported complete object carries its resources. External HTML images currently contribute alt text only; importing linked/data-URI resources needs a separate bounded resource policy and model/UI behavior. OLE, charts, SmartArt, groups and unknown raw object extensions remain outside the basic object clipboard contract and are refused, not silently converted. Native clipboard and PPTX/Office application interoperability require separate acceptance; this change does not claim them.

Additional PPTX/native-format/OPC regression: 124 tests passed. Documentation, formatting, imported-source verification (291 records), engineering gates and whitespace checks passed. Tests were headless; no real desktop clipboard or Office application was exercised.
