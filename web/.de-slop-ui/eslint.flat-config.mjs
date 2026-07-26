import deSlopUi from "./eslint-plugin.mjs";

export default [
  {
    name: "de-slop-ui/guardrails",
    files: ["**/*.{jsx,tsx}"],
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "module",
      parserOptions: {
        ecmaFeatures: {
          jsx: true,
        },
      },
    },
    plugins: {
      "de-slop-ui": deSlopUi,
    },
    rules: {
      "de-slop-ui/accessibility-icon-button-label": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/accessibility-no-focus-ring-removal": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/content-no-placeholder-copy": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/content-no-commented-out-ui": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/content-no-generic-cta-copy": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/clarity-no-over-muted-text": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/clarity-no-clipped-status-text": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/semantics-no-fake-meta-labels": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/semantics-no-fake-lettermark": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/forms-control-needs-accessible-name": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/forms-no-placeholder-only-label": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/structure-no-nested-card": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/structure-no-nested-surface": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/structure-no-metric-strip-card": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/structure-no-surface-grid-overload": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/structure-no-pill-spam": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/interaction-button-without-type": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/interaction-clickable-noninteractive": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/interaction-missing-pointer-affordance": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/interaction-no-empty-href": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/clarity-no-hover-only-legibility": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/clarity-no-vague-alt": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/decorative-no-ambient-orb": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/decorative-no-left-accent-border": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/effects-no-effect-stacking": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/motion-no-transition-all": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/motion-reduced-motion": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/layout-no-h-screen": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ],
      "de-slop-ui/navigation-button-needs-current-state": [
            "warn",
            {
                  "preset": "advisory",
                  "productMode": "data-reporting"
            }
      ]
},
  },
  {
    name: "de-slop-ui/css-guardrails",
    files: ["**/*.{css,scss,sass,pcss}"],
    plugins: {
      "de-slop-ui": deSlopUi,
    },
    processor: "de-slop-ui/css-text",
    rules: {
      "de-slop-ui/decorative-no-left-accent-border": "warn",
    },
  },
];
