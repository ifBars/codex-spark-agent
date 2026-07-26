const JSX_NAMES = new Set(["Card"]);
const PILL_NAMES = new Set(["Badge", "Pill", "Tag", "Chip"]);
const INTERACTIVE_NAMES = new Set(["button", "a"]);
const PLACEHOLDER_PATTERNS = [
  /\blorem ipsum\b/i,
  /\b(john|jane)\s+doe\b/i,
  /\bjane smith\b/i,
  /\bacme\b/i,
  /\bplaceholder\.svg\b/i,
  /\bproduct demo\b/i,
  /\bdQw4w9WgXcQ\b/i,
  /\b(99\.99%|100% uptime|123[-\s]?456[-\s]?7890)\b/i,
];
const GENERIC_CTA_TEXT = new Set([
  "learn more",
  "get started",
  "start building",
  "explore",
  "view details",
  "read more",
  "try it",
  "open",
]);
const SELF_EXPLANATORY_DESTINATION = /\b(docs?|documentation|release|download|github|source|methodology|leaderboard|example|demo|pricing|changelog|quickstart|quick-start)\b/i;
const FAKE_META_PATTERNS = [
  /\b(section|question|step)\s*0?\d+\b/i,
  /\bcase study\s*0?\d+\b/i,
  /\b\d{2}_[a-z0-9_]+\b/i,
  /\b0\d\s+(orchestration|runtime|control|system)\b/i,
  /\b(runtime|operator|control)\s+(layer|status|matrix)\b/i,
  /\b(init_sequence|system_ready|system_failure|critical_failure|arch_chat_module)\b/i,
  /\b(ai context verification|execution protocol|validation protocols|technical proficiency test)\b/i,
  /\b(system online|system processing|system kernel loaded|query the architectural database)\b/i,
  /\b(gemini\s+\d+\s+pro|gemini\s+3\s+pro)\b/i,
  /\blive playground\b/i,
  /\bbuilt for ai product teams\b/i,
];
const VAGUE_ALT = /^(image|photo|picture|screenshot|graphic|illustration)$/i;
const LEFT_ACCENT_CSS_PATTERNS = [
  /box-shadow\s*:\s*inset\s+(?:[2-9]|\d{2,})px\s+0\s+0\s+/i,
  /border-(?:left|inline-start)\s*:\s*(?:[2-9]|\d{2,})px\s+solid\b/i,
  /(?:active|selected|current|checked|pressed)[^{]*\{[^}]*border-(?:left|inline-start)-color\s*:/i,
];

function createRule(meta, create) {
  return {
    meta: {
      type: meta.type || "suggestion",
      docs: {
        description: meta.description,
      },
      schema: [
        {
          type: "object",
          additionalProperties: true,
        },
      ],
      messages: meta.messages,
    },
    create,
  };
}

function jsxName(node) {
  if (!node) return "";
  if (node.type === "JSXIdentifier") return node.name;
  if (node.type === "JSXMemberExpression") return jsxName(node.property);
  return "";
}

function attrValue(node, name) {
  const attr = node.openingElement.attributes.find(
    (item) => item.type === "JSXAttribute" && jsxName(item.name) === name,
  );
  if (!attr || !attr.value) return null;
  if (attr.value.type === "Literal") return String(attr.value.value || "");
  if (attr.value.type === "JSXExpressionContainer" && attr.value.expression.type === "Literal") {
    return String(attr.value.expression.value || "");
  }
  return null;
}

function hasAttr(node, name) {
  return node.openingElement.attributes.some(
    (item) => item.type === "JSXAttribute" && jsxName(item.name) === name,
  );
}

function hasEventAttr(node, name) {
  return hasAttr(node, name);
}

function classText(node) {
  return attrValue(node, "className") || attrValue(node, "class") || "";
}

function textFromNode(node) {
  if (!node) return "";
  if (node.type === "JSXText") return node.value;
  if (node.type === "Literal") return String(node.value || "");
  if (node.type === "TemplateElement") return node.value.raw || "";
  return "";
}

function combinedText(node) {
  let text = "";
  function visit(current) {
    if (!current) return;
    if (current.type === "JSXText") {
      text += current.value;
      return;
    }
    if (current.type === "Literal" && typeof current.value === "string") {
      text += current.value;
      return;
    }
    if (current.type === "JSXExpressionContainer") {
      visit(current.expression);
      return;
    }
    if (Array.isArray(current.children)) {
      current.children.forEach(visit);
    }
  }
  visit(node);
  return text.trim().replace(/\s+/g, " ");
}

function hasClass(node, pattern) {
  return pattern.test(classText(node));
}

function hasLeftAccentCss(text) {
  if (!text) return false;
  return LEFT_ACCENT_CSS_PATTERNS.some((pattern) => pattern.test(text));
}

function hasNamedDescendant(node, names) {
  let found = false;
  function visit(current) {
    if (!current || found) return;
    if (current.type === "JSXElement") {
      if (names.has(jsxName(current.openingElement.name))) {
        found = true;
        return;
      }
      current.children.forEach(visit);
    }
  }
  node.children.forEach(visit);
  return found;
}

function hasAccessibleName(node) {
  if (attrValue(node, "aria-label") || attrValue(node, "aria-labelledby") || attrValue(node, "title")) {
    return true;
  }
  if (combinedText(node)) return true;
  return node.children.some(
    (child) => child.type === "JSXElement" && /\bsr-only\b/.test(classText(child)),
  );
}

function interactiveDestination(node) {
  return attrValue(node, "href") || attrValue(node, "to") || "";
}

function isSurface(node) {
  if (!node || node.type !== "JSXElement") return false;
  const classes = classText(node);
  if (!classes) return false;
  const hasFrame = /\brounded(?:-|$|\[)/.test(classes) && /\bborder(?:-|$)/.test(classes);
  const hasSurfaceColor = /\bbg-(?:card|surface|secondary|slate|zinc|neutral|white|black|background|muted|\[)/.test(classes);
  const hasPadding = /\b(?:p|px|py)-/.test(classes);
  return hasFrame && hasSurfaceColor && hasPadding;
}

function isMetricLike(node) {
  if (!node || node.type !== "JSXElement") return false;
  const text = combinedText(node);
  const classes = classText(node);
  const hasNumber = /[-+]?\d+(?:\.\d+)?%?/.test(text);
  const hasStatusWord = /\b(running|idle|paused|active|ready|complete|failed|queued|loading|stopped|online|offline)\b/i.test(text);
  const hasMicroLabel =
    /\buppercase\b/.test(classes) ||
    /\btracking-\[/.test(classes) ||
    /\btext-\[(?:7|8|9|10|11)px\]/.test(classes) ||
    /\btext-xs\b/.test(classes);
  const hasEmphasis =
    /\btext-(?:xl|2xl|3xl|4xl)\b/.test(classes) ||
    /\bfont-(?:bold|black|semibold|extrabold)\b/.test(classes);
  return (hasNumber || hasStatusWord) && (hasMicroLabel || hasEmphasis);
}

function insideSurface(node) {
  let current = node.parent;
  while (current) {
    if (isSurface(current)) return true;
    current = current.parent;
  }
  return false;
}

function insideElementNamed(node, name) {
  let current = node.parent;
  while (current) {
    if (current.type === "JSXElement" && jsxName(current.openingElement.name) === name) {
      return true;
    }
    current = current.parent;
  }
  return false;
}

function isInsideNativeLabel(node) {
  return insideElementNamed(node, "label");
}

function insideNamedElement(node, names) {
  let current = node.parent;
  while (current) {
    if (current.type === "JSXElement" && names.has(jsxName(current.openingElement.name))) {
      return true;
    }
    current = current.parent;
  }
  return false;
}

function reportTextPattern(context, node, patterns, messageId) {
  const text = textFromNode(node).trim().replace(/\s+/g, " ");
  if (!text) return;
  if (patterns.some((pattern) => pattern.test(text))) {
    context.report({ node, messageId });
  }
}

const rules = {
  "accessibility-icon-button-label": createRule(
    {
      type: "problem",
      description: "Flag icon-only interactive elements without an accessible name.",
      messages: {
        iconOnly:
          "Icon-only controls need an accessible name through visible text, aria-label, aria-labelledby, title, or sr-only text.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const name = jsxName(node.openingElement.name);
        if (!INTERACTIVE_NAMES.has(name)) return;
        if (hasAccessibleName(node)) return;
        const hasIcon = hasNamedDescendant(node, new Set(["svg"])) || node.children.some(
          (child) => child.type === "JSXElement" && /^[A-Z]/.test(jsxName(child.openingElement.name)),
        );
        if (hasIcon) {
          context.report({ node: node.openingElement.name, messageId: "iconOnly" });
        }
      },
    }),
  ),

  "accessibility-no-focus-ring-removal": createRule(
    {
      type: "problem",
      description: "Flag removed focus styling without another visible focus affordance.",
      messages: {
        noFocus:
          "Do not remove focus outlines/rings without adding a visible replacement focus style.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const classes = classText(node);
        if (!classes) return;
        const removesFocus = /\b(?:focus|focus-visible):(?:outline-none|ring-0)\b|\boutline-none\b/.test(classes);
        const replacement = /\b(?:focus|focus-visible):(?!ring-0)(?:ring|outline|border|shadow|bg-|text-)/.test(classes);
        if (removesFocus && !replacement) {
          context.report({ node: node.openingElement.name, messageId: "noFocus" });
        }
      },
    }),
  ),

  "content-no-placeholder-copy": createRule(
    {
      description: "Flag obvious placeholder copy and fake AI demo data.",
      messages: {
        placeholder:
          "Avoid placeholder or fake-looking demo content. Use product-specific copy or realistic sample data.",
      },
    },
    (context) => ({
      JSXText(node) {
        reportTextPattern(context, node, PLACEHOLDER_PATTERNS, "placeholder");
      },
      Literal(node) {
        if (typeof node.value === "string") {
          reportTextPattern(context, node, PLACEHOLDER_PATTERNS, "placeholder");
        }
      },
      TemplateElement(node) {
        reportTextPattern(context, node, PLACEHOLDER_PATTERNS, "placeholder");
      },
    }),
  ),

  "content-no-commented-out-ui": createRule(
    {
      description: "Flag commented-out JSX or scaffold residue.",
      messages: {
        commented:
          "Remove commented-out UI before shipping. Finished interfaces should not carry scaffold residue.",
      },
    },
    (context) => ({
      Program() {
        const source = context.sourceCode || context.getSourceCode();
        for (const comment of source.getAllComments()) {
          if (/<\/?[A-Z_a-z][^>]*>/.test(comment.value) || /\bTODO\b|\bFIXME\b|console\.log/.test(comment.value)) {
            context.report({ loc: comment.loc, messageId: "commented" });
          }
        }
      },
    }),
  ),

  "content-no-generic-cta-copy": createRule(
    {
      description: "Flag generic CTA labels that lack product or destination context.",
      messages: {
        genericCta:
          "This CTA label is generic. Add visible product/action context, aria-label/title context, or point it at a self-explanatory destination such as docs, releases, source, examples, downloads, or methodology.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const name = jsxName(node.openingElement.name);
        const isInteractive =
          INTERACTIVE_NAMES.has(name) ||
          name === "Link" ||
          name === "NavLink" ||
          hasAttr(node, "onClick");
        if (!isInteractive) return;
        const text = combinedText(node).toLowerCase().replace(/\s+/g, " ").trim();
        if (!GENERIC_CTA_TEXT.has(text)) return;
        if (attrValue(node, "aria-label") || attrValue(node, "title")) return;
        if (SELF_EXPLANATORY_DESTINATION.test(interactiveDestination(node))) return;
        context.report({ node: node.openingElement.name, messageId: "genericCta" });
      },
    }),
  ),

  "clarity-no-over-muted-text": createRule(
    {
      description: "Flag visible text made too faint through opacity-heavy classes.",
      messages: {
        muted:
          "This text is heavily muted. Important labels, controls, and body copy should remain readable in the resting state.",
      },
    },
    (context) => ({
      JSXElement(node) {
        if (attrValue(node, "aria-hidden") === "true") return;
        if (!combinedText(node)) return;
        const classes = classText(node);
        const overMuted =
          /\btext-[^\s/]+\/(?:[0-4]?\d|50)\b/.test(classes) ||
          /\btext-\[[^\]]+\]\/(?:[0-4]?\d|50)\b/.test(classes) ||
          /\bopacity-(?:0|10|20|25|30|40)\b/.test(classes) ||
          /\bopacity-\[(?:0?\.[0-4]\d?|\.?[0-4]\d?)\]\b/.test(classes);
        if (overMuted) {
          context.report({ node: node.openingElement.name, messageId: "muted" });
        }
      },
    }),
  ),

  "clarity-no-clipped-status-text": createRule(
    {
      description: "Flag large status/value text likely to clip inside constrained panels.",
      messages: {
        clipped:
          "Large status/value text inside a constrained or overflow-hidden surface can clip. Give it room, wrap intentionally, or use a more compact status treatment.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const text = combinedText(node);
        if (!/\b(running|idle|paused|active|ready|complete|failed|queued|loading|stopped|online|offline)\b/i.test(text)) return;
        const classes = classText(node);
        const bigText = /\btext-(?:xl|2xl|3xl|4xl|5xl)\b|\btext-\[(?:2|3|4)\dpx\]/.test(classes);
        const noWrap = /\bwhitespace-nowrap\b|\btruncate\b|\boverflow-hidden\b/.test(classes);
        const emphasized = /\buppercase\b|\bfont-(?:bold|black|extrabold)\b|\btracking-\[/.test(classes);
        if ((bigText && emphasized) || (bigText && noWrap)) {
          context.report({ node: node.openingElement.name, messageId: "clipped" });
        }
      },
    }),
  ),

  "semantics-no-fake-meta-labels": createRule(
    {
      description: "Flag decorative pseudo-structure labels.",
      messages: {
        fakeMeta:
          "This looks like decorative pseudo-structure. Use labels that explain real state, scope, or hierarchy.",
      },
    },
    (context) => ({
      JSXText(node) {
        reportTextPattern(context, node, FAKE_META_PATTERNS, "fakeMeta");
      },
      Literal(node) {
        if (typeof node.value === "string") {
          reportTextPattern(context, node, FAKE_META_PATTERNS, "fakeMeta");
        }
      },
    }),
  ),

  "semantics-no-fake-lettermark": createRule(
    {
      description: "Flag small rounded boxes that use arbitrary initials as fake logo/avatar marks.",
      messages: {
        lettermark:
          "This looks like a generic AI lettermark. Use a real logo, avatar data, meaningful icon, or plain text label.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const text = combinedText(node);
        if (!/^[A-Z0-9]{1,3}$/.test(text)) return;
        const classes = classText(node);
        const fixedMark = /\b(?:size|h|w)-(?:6|7|8|9|10|11|12)\b/.test(classes) || /\b(?:h|w)-\[[^\]]+\]/.test(classes);
        if (fixedMark && /\brounded(?:-|$|\[)/.test(classes) && /\b(?:border|bg-)/.test(classes)) {
          context.report({ node: node.openingElement.name, messageId: "lettermark" });
        }
      },
    }),
  ),

  "forms-no-placeholder-only-label": createRule(
    {
      type: "problem",
      description: "Flag form controls that rely on placeholder text as the only label.",
      messages: {
        placeholderOnly:
          "Do not use placeholder text as the only label. Add a label, aria-label, aria-labelledby, or an id tied to a visible label.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const name = jsxName(node.openingElement.name);
        if (!["input", "textarea", "select"].includes(name)) return;
        if (!attrValue(node, "placeholder")) return;
        if (attrValue(node, "aria-label") || attrValue(node, "aria-labelledby") || attrValue(node, "id")) return;
        context.report({ node: node.openingElement.name, messageId: "placeholderOnly" });
      },
    }),
  ),

  "forms-control-needs-accessible-name": createRule(
    {
      type: "problem",
      description: "Flag user-facing form and demo controls without an accessible name.",
      messages: {
        unnamed:
          "Controls need a semantic name through a label, id, aria-label, aria-labelledby, or title. Visual annotations around demos do not label the control.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const name = jsxName(node.openingElement.name);
        if (!["input", "textarea", "select"].includes(name)) return;
        const type = (attrValue(node, "type") || "").toLowerCase();
        if (["hidden", "submit", "button", "reset", "image"].includes(type)) return;
        if (
          attrValue(node, "aria-label") ||
          attrValue(node, "aria-labelledby") ||
          attrValue(node, "title") ||
          hasAttr(node, "id") ||
          isInsideNativeLabel(node)
        ) {
          return;
        }
        context.report({ node: node.openingElement.name, messageId: "unnamed" });
      },
    }),
  ),

  "structure-no-nested-card": createRule(
    {
      description: "Flag Card inside Card nesting.",
      messages: {
        nestedCard:
          "Avoid Card inside Card unless the nested surface has a distinct product job. Use spacing, dividers, or sections first.",
      },
    },
    (context) => ({
      JSXElement(node) {
        if (JSX_NAMES.has(jsxName(node.openingElement.name)) && insideNamedElement(node, JSX_NAMES)) {
          context.report({ node: node.openingElement.name, messageId: "nestedCard" });
        }
      },
    }),
  ),

  "structure-no-nested-surface": createRule(
    {
      description: "Flag generic framed surface nesting based on classes.",
      messages: {
        nestedSurface:
          "This looks like a generic surface nested inside another surface. Use nesting only when the inner frame has a distinct product job.",
      },
    },
    (context) => ({
      JSXElement(node) {
        if (isSurface(node) && insideSurface(node)) {
          context.report({ node: node.openingElement.name, messageId: "nestedSurface" });
        }
      },
    }),
  ),

  "structure-no-metric-strip-card": createRule(
    {
      description: "Flag generic KPI/stat strips inside one framed surface.",
      messages: {
        metricStrip:
          "This looks like a generic metric strip. Check whether these values would scan better as rows, a chart legend, aligned key-value groups, or inline summary text.",
      },
    },
    (context) => ({
      JSXElement(node) {
        if (!isSurface(node)) return;
        const metricChildren = node.children.filter((child) => isMetricLike(child));
        const text = combinedText(node);
        const metricWords = /\b(best|average|fitness|diversity|status|score|rate|total|mean|variance|state|count)\b/i.test(text);
        if (metricChildren.length >= 3 && metricWords) {
          context.report({ node: node.openingElement.name, messageId: "metricStrip" });
        }
      },
    }),
  ),

  "structure-no-surface-grid-overload": createRule(
    {
      description: "Flag groups with too many equal framed surfaces.",
      messages: {
        overload:
          "This many similar framed surfaces often reads as AI card-grid filler. Check whether hierarchy, sections, or tables would communicate better.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const surfaceChildren = node.children.filter((child) => isSurface(child));
        if (surfaceChildren.length >= 6) {
          context.report({ node: node.openingElement.name, messageId: "overload" });
        }
      },
    }),
  ),

  "structure-no-pill-spam": createRule(
    {
      description: "Flag dense sibling groups of Badge, Pill, Tag, or Chip components.",
      messages: {
        pillSpam:
          "This many pill-like components often reads as decorative filler. Keep pills for state, filters, or compact real metadata.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const pillChildren = node.children.filter(
          (child) =>
            child.type === "JSXElement" && PILL_NAMES.has(jsxName(child.openingElement.name)),
        );
        if (pillChildren.length >= 5) {
          context.report({ node: node.openingElement.name, messageId: "pillSpam" });
        }
      },
    }),
  ),

  "interaction-button-without-type": createRule(
    {
      type: "problem",
      description: "Flag buttons without explicit type.",
      messages: {
        noType:
          "Buttons should declare type=\"button\", type=\"submit\", or type=\"reset\" so behavior is intentional.",
      },
    },
    (context) => ({
      JSXElement(node) {
        if (jsxName(node.openingElement.name) === "button" && !hasAttr(node, "type")) {
          context.report({ node: node.openingElement.name, messageId: "noType" });
        }
      },
    }),
  ),

  "interaction-clickable-noninteractive": createRule(
    {
      type: "problem",
      description: "Flag clickable div/span elements without keyboard semantics.",
      messages: {
        clickable:
          "Clickable non-interactive elements need button/link semantics or role, tabIndex, and keyboard handling.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const name = jsxName(node.openingElement.name);
        if (!["div", "span"].includes(name)) return;
        if (!hasEventAttr(node, "onClick")) return;
        const hasKeyboard = hasEventAttr(node, "onKeyDown") || hasEventAttr(node, "onKeyUp");
        if (!hasAttr(node, "role") || !hasAttr(node, "tabIndex") || !hasKeyboard) {
          context.report({ node: node.openingElement.name, messageId: "clickable" });
        }
      },
    }),
  ),

  "interaction-missing-pointer-affordance": createRule(
    {
      description: "Flag interactive elements with hover styling but no pointer cursor.",
      messages: {
        pointer:
          "Interactive elements with hover styling should usually include cursor-pointer so they read as clickable.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const name = jsxName(node.openingElement.name);
        const interactive = INTERACTIVE_NAMES.has(name) || hasAttr(node, "onClick");
        if (!interactive) return;
        const classes = classText(node);
        if (/\b(?:hover|group-hover):/.test(classes) && !/\bcursor-pointer\b/.test(classes)) {
          context.report({ node: node.openingElement.name, messageId: "pointer" });
        }
      },
    }),
  ),

  "interaction-no-empty-href": createRule(
    {
      type: "problem",
      description: "Flag dead placeholder links.",
      messages: {
        emptyHref: "Avoid href=\"#\" placeholder links. Use a real destination or disable the action.",
      },
    },
    (context) => ({
      JSXElement(node) {
        if (jsxName(node.openingElement.name) !== "a") return;
        if (attrValue(node, "href") === "#") {
          context.report({ node: node.openingElement.name, messageId: "emptyHref" });
        }
      },
    }),
  ),

  "clarity-no-hover-only-legibility": createRule(
    {
      description: "Flag class strings that hide important content until hover.",
      messages: {
        hoverOnly:
          "Content appears hidden until hover. Important text and controls should be legible in the resting state.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const classes = classText(node);
        if (!classes) return;
        const hides = /\b(opacity-0|text-transparent|invisible)\b/.test(classes);
        const reveals = /\b(group-hover:|hover:)(opacity-100|text-|visible)/.test(classes);
        if (hides && reveals) {
          context.report({ node: node.openingElement.name, messageId: "hoverOnly" });
        }
      },
    }),
  ),

  "clarity-no-vague-alt": createRule(
    {
      type: "problem",
      description: "Flag vague image alt text.",
      messages: {
        vagueAlt:
          "Alt text is too vague. Describe the image content, or use an empty alt only for decorative images.",
      },
    },
    (context) => ({
      JSXElement(node) {
        if (!["img", "Image"].includes(jsxName(node.openingElement.name))) return;
        const alt = attrValue(node, "alt");
        if (alt && VAGUE_ALT.test(alt.trim())) {
          context.report({ node: node.openingElement.name, messageId: "vagueAlt" });
        }
      },
    }),
  ),

  "decorative-no-ambient-orb": createRule(
    {
      description: "Flag common decorative gradient/blob/orb background elements.",
      messages: {
        orb:
          "This looks like a decorative ambient orb/blob. Use background treatment only when it supports the product, hierarchy, or readability.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const classes = classText(node);
        const ambient =
          /\bpointer-events-none\b/.test(classes) &&
          /\b(?:absolute|fixed)\b/.test(classes) &&
          /\brounded-full\b/.test(classes) &&
          /\bblur-/.test(classes) &&
          /\bbg-/.test(classes);
        if (ambient) {
          context.report({ node: node.openingElement.name, messageId: "orb" });
        }
      },
    }),
  ),

  "decorative-no-left-accent-border": createRule(
    {
      description: "Flag decorative left accent borders, rails, and inset stripes.",
      messages: {
        leftAccent:
          "Avoid left accent borders, rails, or inset stripes for selected/active UI. Use full-row background, border-color, text/icon weight, checkmarks, or semantic current/selected state instead.",
      },
    },
    (context) => {
      let reportedSourceText = false;

      function reportText(node, text) {
        if (reportedSourceText || !hasLeftAccentCss(text)) return;
        reportedSourceText = true;
        context.report({ node, messageId: "leftAccent" });
      }

      return {
        Program(node) {
          const source = context.sourceCode || context.getSourceCode();
          reportText(node, source.getText());
        },
        Literal(node) {
          if (typeof node.value === "string") {
            reportText(node, node.value);
          }
        },
        TemplateElement(node) {
          reportText(node, node.value.raw || node.value.cooked || "");
        },
        JSXElement(node) {
          const name = jsxName(node.openingElement.name);
          if (name === "blockquote") return;
          if (attrValue(node, "role") && /^(alert|status|note|progressbar)$/.test(attrValue(node, "role"))) return;
          const classes = classText(node);
          const hasLeftBorder = /\bborder-(?:l|s)(?:-(?:2|3|4|8|\[[^\]]+\]))?\b/.test(classes);
          const hasAccentColor = /\bborder-(?:(?:l|s)-)?(?:primary|accent|cyan|sky|blue|emerald|green|lime|amber|yellow|orange|rose|red|violet|purple|fuchsia|pink)(?:-|\/|\b|\[)/.test(classes);
          const hasInsetLeftShadow = /\b(?:shadow-\[|\[box-shadow:)[^\]\s]*inset[_-](?:[2-9]|\d{2,})px[_-]0[_-]0/i.test(classes);
          const looksLikeUiSurface = /\b(?:p|px|py)-/.test(classes) || /\bbg-/.test(classes) || /\bborder\b/.test(classes) || /\brounded/.test(classes);
          if (((hasLeftBorder && hasAccentColor) || hasInsetLeftShadow) && looksLikeUiSurface) {
            context.report({ node: node.openingElement.name, messageId: "leftAccent" });
          }
        },
      };
    },
  ),

  "effects-no-effect-stacking": createRule(
    {
      description: "Flag heavy decorative effect stacking.",
      messages: {
        effectStack:
          "This element stacks several decorative effects. Make sure depth comes from hierarchy, not blur, glow, gradient, and translucency piled together.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const classes = classText(node);
        if (!classes) return;
        const signals = [
          /\bbackdrop-blur|\bblur-/.test(classes),
          /\bbg-gradient|\bfrom-|\bvia-|\bto-/.test(classes),
          /\bshadow|\bdrop-shadow|\bglow/.test(classes),
          /\bbg-[^\s]+\/\d+|\bopacity-\d+/.test(classes),
          /\brounded-(2xl|3xl|full)|rounded-\[/.test(classes),
        ].filter(Boolean).length;
        if (signals >= 4) {
          context.report({ node: node.openingElement.name, messageId: "effectStack" });
        }
      },
    }),
  ),

  "motion-no-transition-all": createRule(
    {
      description: "Flag transition-all on JSX elements.",
      messages: {
        transitionAll:
          "Avoid transition-all. Transition the intended property, such as colors, opacity, transform, or shadow.",
      },
    },
    (context) => ({
      JSXElement(node) {
        if (hasClass(node, /\btransition-all\b/)) {
          context.report({ node: node.openingElement.name, messageId: "transitionAll" });
        }
      },
    }),
  ),

  "motion-reduced-motion": createRule(
    {
      type: "problem",
      description: "Flag obvious Tailwind animations without reduced-motion handling.",
      messages: {
        reducedMotion:
          "Animation utilities should include motion-safe or motion-reduce handling so motion preference is respected.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const classes = classText(node);
        if (!classes) return;
        const animates = /\banimate-|\btransition-\[|\bduration-\[/.test(classes);
        const handlesMotion = /\bmotion-safe:|\bmotion-reduce:/.test(classes);
        if (animates && !handlesMotion) {
          context.report({ node: node.openingElement.name, messageId: "reducedMotion" });
        }
      },
    }),
  ),

  "layout-no-h-screen": createRule(
    {
      type: "problem",
      description: "Flag h-screen usage in JSX classes.",
      messages: {
        hScreen:
          "Avoid h-screen for full-height UI. Prefer min-h-dvh, min-h-svh, or the repo's safer viewport token.",
      },
    },
    (context) => ({
      JSXElement(node) {
        const classes = classText(node);
        if (/\bh-screen\b/.test(classes)) {
          context.report({ node: node.openingElement.name, messageId: "hScreen" });
        }
      },
    }),
  ),

  "navigation-button-needs-current-state": createRule(
    {
      description: "Flag navigation buttons without semantic current/pressed state.",
      messages: {
        navState:
          "Buttons inside nav should expose current state with aria-current or aria-pressed, or use links for route navigation.",
      },
    },
    (context) => ({
      JSXElement(node) {
        if (jsxName(node.openingElement.name) !== "button") return;
        if (!insideElementNamed(node, "nav")) return;
        if (!hasAttr(node, "aria-current") && !hasAttr(node, "aria-pressed")) {
          context.report({ node: node.openingElement.name, messageId: "navState" });
        }
      },
    }),
  ),
};

const cssTextProcessor = {
  meta: {
    name: "de-slop-ui/css-text",
  },
  preprocess(text) {
    return ["const __deSlopUiCssText = " + JSON.stringify(text) + ";\n"];
  },
  postprocess(messageLists) {
    return messageLists.flat();
  },
  supportsAutofix: false,
};

export default { rules, processors: { "css-text": cssTextProcessor } };
