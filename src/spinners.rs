#[derive(Debug, Clone, Copy)]
pub(crate) struct ApplicationSpinner {
    pub(crate) label: &'static str,
    pub(crate) example_tool: &'static str,
    pub(crate) set_name: &'static str,
}

pub(crate) const SPARK_PULSE: &[&str] = &["·", "•", "●", "•"];
pub(crate) const SPARK_ORBIT: &[&str] = &["◐", "◓", "◑", "◒"];
pub(crate) const SPARK_SCAN: &[&str] = &[
    "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█", "▉", "▊", "▋", "▌", "▍", "▎",
];
pub(crate) const SPARK_WAVE: &[&str] = &[
    "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█", "▇", "▆", "▅", "▄", "▃", "▂",
];
pub(crate) const SPARK_DIAMOND: &[&str] = &["◇", "◈", "◆", "◈"];
pub(crate) const SPARK_FLIP: &[&str] = &["◢", "◣", "◤", "◥"];
pub(crate) const SPARK_COMET: &[&str] = &["⠁", "⠂", "⠄", "⡀", "⢀", "⠠", "⠐", "⠈"];
pub(crate) const SPARK_TWIN: &[&str] = &["◜", "◝", "◞", "◟"];

pub(crate) const APPLICATION_SPINNERS: [ApplicationSpinner; 11] = [
    ApplicationSpinner {
        label: "file read",
        example_tool: "fs.read",
        set_name: "SPARK_PULSE",
    },
    ApplicationSpinner {
        label: "file list/stat",
        example_tool: "fs.list",
        set_name: "SPARK_ORBIT",
    },
    ApplicationSpinner {
        label: "file search",
        example_tool: "fs.search",
        set_name: "SPARK_SCAN",
    },
    ApplicationSpinner {
        label: "file write",
        example_tool: "fs.write",
        set_name: "SPARK_WAVE",
    },
    ApplicationSpinner {
        label: "file edit/replace",
        example_tool: "fs.edit",
        set_name: "SPARK_DIAMOND",
    },
    ApplicationSpinner {
        label: "file rename",
        example_tool: "fs.rename",
        set_name: "SPARK_FLIP",
    },
    ApplicationSpinner {
        label: "command",
        example_tool: "cmd.exec",
        set_name: "ARROW",
    },
    ApplicationSpinner {
        label: "browser",
        example_tool: "browser.run",
        set_name: "DOUBLE_ARROW",
    },
    ApplicationSpinner {
        label: "web search",
        example_tool: "web.search",
        set_name: "OGHAM_A",
    },
    ApplicationSpinner {
        label: "subagent",
        example_tool: "subagent.run",
        set_name: "SPARK_TWIN",
    },
    ApplicationSpinner {
        label: "MCP",
        example_tool: "mcp__server__tool",
        set_name: "CANADIAN",
    },
];

pub(crate) fn tool_spinner_frame(tool_name: Option<&str>, tick: usize) -> &'static str {
    let symbols = tool_spinner_symbols(tool_name);
    symbols[tick % symbols.len()]
}

fn tool_spinner_symbols(tool_name: Option<&str>) -> &'static [&'static str] {
    match tool_name {
        Some("fs.read") => SPARK_PULSE,
        Some("fs.list" | "fs.stat") => SPARK_ORBIT,
        Some("fs.search") => SPARK_SCAN,
        Some("fs.write") => SPARK_WAVE,
        Some("fs.replace" | "fs.edit") => SPARK_DIAMOND,
        Some("fs.rename") => SPARK_FLIP,
        Some("cmd.exec") => throbber_widgets_tui::ARROW.symbols,
        Some("browser.run") => throbber_widgets_tui::DOUBLE_ARROW.symbols,
        Some("web.search") => throbber_widgets_tui::OGHAM_A.symbols,
        Some("subagent.run") => SPARK_TWIN,
        Some(name) if name.starts_with("mcp__") => throbber_widgets_tui::CANADIAN.symbols,
        _ => throbber_widgets_tui::BRAILLE_SIX.symbols,
    }
}

#[cfg(test)]
mod tests {
    use super::{APPLICATION_SPINNERS, tool_spinner_frame};

    #[test]
    fn application_spinner_examples_match_their_declared_sets() {
        for spinner in APPLICATION_SPINNERS {
            let expected = match spinner.set_name {
                "SPARK_PULSE" => super::SPARK_PULSE,
                "SPARK_ORBIT" => super::SPARK_ORBIT,
                "SPARK_SCAN" => super::SPARK_SCAN,
                "SPARK_WAVE" => super::SPARK_WAVE,
                "SPARK_DIAMOND" => super::SPARK_DIAMOND,
                "SPARK_FLIP" => super::SPARK_FLIP,
                "ARROW" => throbber_widgets_tui::ARROW.symbols,
                "DOUBLE_ARROW" => throbber_widgets_tui::DOUBLE_ARROW.symbols,
                "OGHAM_A" => throbber_widgets_tui::OGHAM_A.symbols,
                "SPARK_TWIN" => super::SPARK_TWIN,
                "CANADIAN" => throbber_widgets_tui::CANADIAN.symbols,
                unknown => panic!("unrecognized spinner set {unknown}"),
            };
            assert_eq!(
                tool_spinner_frame(Some(spinner.example_tool), 0),
                expected[0],
                "{} should use {}",
                spinner.example_tool,
                spinner.set_name
            );
        }
    }

    #[test]
    fn unknown_tools_keep_the_general_tool_spinner() {
        assert_eq!(
            tool_spinner_frame(Some("future.tool"), 0),
            throbber_widgets_tui::BRAILLE_SIX.symbols[0]
        );
    }
}
