# JD-307: keep pasted media embedded

2026-11-06. This is an object-transfer resource fix, not complete Office clipboard support.

A destination resource with an external link and identical cached bytes could be reused by PasteObjects, turning an embedded source image into a linked destination image. The new regression failed before the fix: the pasted image reused the linked resource ID. The lower-level Presentation::add_media also deduplicated by bytes alone, ignoring both link status and content type.

Both lookup layers now reuse only embedded resources with matching content type and bytes. Repeated pastes still deduplicate the correct embedded resource; the linked resource and a same-byte/different-type resource remain intact. The regression checks two pastes, resource identity and bytes, link absence, unchanged destination links, undo back to the exact initial model and redo. Its bytes and unavailable file URL are synthetic; no URL is opened or external resource fetched.

Model/engine/OPC tests passed (144 tests), as did strict library Clippy. Adapted source fingerprints were updated for the two imported files while preserving original source fingerprints.

Rich text still cannot transfer image-resource bytes: cross-document text with unresolved picture formatting is conservatively rejected, while copying a supported complete object carries its resources. External HTML images currently contribute alt text only; importing linked/data-URI resources needs a separate bounded resource policy and model/UI behavior. OLE, charts, SmartArt, groups and unknown raw object extensions remain outside the basic object clipboard contract and are refused, not silently converted. Native clipboard and PPTX/Office application interoperability require separate acceptance; this change does not claim them.

Additional PPTX/native-format/OPC regression: 124 tests passed. Documentation, formatting, imported-source verification (291 records), engineering gates and whitespace checks passed. Tests were headless; no real desktop clipboard or Office application was exercised.

## Staged text and resource transaction

A text-only SetText with foreign picture-bullet/fill IDs can bind to unrelated destination media when IDs collide. A regression demonstrates this contract limitation. The new text_resources capture/prepare API and SetTextWithResources command stage a complete final text draft with its embedded image bytes, remap all typed media references, and commit text and resources as one history entry. Dropping a prepared command cancels without mutation. Execution revalidates resource completeness, uniqueness, image MIME category, nonempty bytes, 512-resource/16-MiB budgets, target body and active slide. External linked source media is refused without reading the link. Session staging also rolls back allocations if a subsequent batch operation fails.

Six transaction tests cover the ID collision, picture fill plus picture bullet, cancellation, missing/duplicate/unused/non-image resources, linked-source refusal, byte budget, changed slide, stale body, failure after allocation, notes/shape/cell targets, serialized command roundtrip and undo/redo. Image bytes are synthetic and are not decoded or executed.

This is an engine primitive, not a new clipboard protocol or UI integration. All references in the complete final draft must use the supplied resource namespace; the caller must reconcile mixed source/destination drafts and scope document-local links and document identity. Guards check body content and active slide, not an immutable document revision token. Image decoding and pixel limits remain the caller's responsibility. Existing native rich-text clipboard behavior remains conservative; no inline image conversion, external resource fetching, Office interoperability or real-window acceptance is claimed.

Transaction validation: 150 model/engine/OPC tests passed; strict all-target Clippy passed for those crates. Desktop, shell and CLI consumer checks passed. Formatting, documentation, 291 source fingerprints, engineering gates and whitespace checks passed. No actual native clipboard was accessed.

## Mixed-source draft namespace

The engine now provides a private `text_resources::Draft` initialized from the target text and its embedded resources. Each paste validates its own resource envelope, assigns noncolliding draft IDs (deduplicating only identical bytes and content type), remaps the fragment before insertion, and retains only resources referenced by the resulting complete draft. The original target guard survives every paste. Failed insertion/budget validation leaves the previous draft unchanged; dropping it cancels all staged pastes; finishing produces one atomic undoable command.

Four additional regressions cover a destination and two sources sharing the same original ID but different bytes, repeated paste deduplication, cancellation, single undo/redo, invalid range/missing resources/combined budget rollback, stale target refusal, inherited picture formatting and pruning unused incoming paragraph resources. Existing rich-text insertion semantics are deliberately reused: first-paragraph formatting and inherited run formatting are not redefined by this API. Tests use inert synthetic bytes and never open links or decode/execute content.

This resolves mixed-source numbering for this engine draft API, not native clipboard or UI integration. The host still needs a resource-bearing clipboard envelope, verified document/link scoping, draft lifecycle and user-facing failure handling. Existing linked target resources are conservatively refused at draft initialization. No new inline-image representation or conversion is introduced.

Mixed-source validation: all 154 model/engine/OPC tests passed (10 text-resource transaction tests), strict all-target Clippy and desktop/shell/CLI checks passed, as did formatting, documentation, source fingerprints and engineering gates. No real-window acceptance was performed.
