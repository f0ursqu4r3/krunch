// Render streamed model text as sanitized markdown HTML. Model output is
// untrusted (it can contain arbitrary HTML/script), so every render is passed
// through DOMPurify before it reaches `v-html`. `marked` is synchronous and
// cheap enough to re-run on each stream tick.

import { marked } from "marked";
import DOMPurify from "dompurify";

marked.setOptions({
  gfm: true,
  breaks: true, // single newlines become <br>, matching the old pre-wrap feel
});

/** Parse `text` to sanitized HTML. Returns "" for empty/nullish input. */
export function renderMarkdown(text: string | null | undefined): string {
  if (!text) return "";
  const html = marked.parse(text, { async: false });
  return DOMPurify.sanitize(html);
}
