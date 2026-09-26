import { HighlightStyle } from "@codemirror/language";
import { tags } from "@lezer/highlight";

// Shared Tint syntax palette used by both the landing editor and sandbox.
export const darkHighlightColors = HighlightStyle.define([
  { tag: [tags.keyword, tags.operatorKeyword], color: "#a29bfe" },
  { tag: tags.string, color: "#8fd19e" },
  { tag: [tags.number, tags.atom, tags.bool], color: "#f5a623" },
  { tag: tags.typeName, color: "#7db8ff" },
  { tag: tags.function(tags.variableName), color: "#7db8ff" },
  { tag: tags.comment, color: "#7a7a7a", fontStyle: "italic" },
  { tag: [tags.operator, tags.punctuation], color: "#9a9a9a" },
]);

export const lightHighlightColors = HighlightStyle.define([
  { tag: [tags.keyword, tags.operatorKeyword], color: "#6c5ce7" },
  { tag: tags.string, color: "#218739" },
  { tag: [tags.number, tags.atom, tags.bool], color: "#b45309" },
  { tag: tags.typeName, color: "#1769aa" },
  { tag: tags.function(tags.variableName), color: "#1769aa" },
  { tag: tags.comment, color: "#777777", fontStyle: "italic" },
  { tag: [tags.operator, tags.punctuation], color: "#777777" },
]);
