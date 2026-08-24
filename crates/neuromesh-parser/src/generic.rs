use crate::calls::{brace_delta, extract_calls_from_line, extract_type_uses_from_line};
use crate::types::{AstAnalysisResult, ParsedImport, ParsedRelationship, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub struct GenericParser;

impl GenericParser {
    pub fn parse(file_path: &Path, content: &str) -> AstAnalysisResult {
        let mut result = AstAnalysisResult::default();
        let filename = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module");

        static CLASS_RE: OnceLock<Regex> = OnceLock::new();
        static IMPORT_RE: OnceLock<Regex> = OnceLock::new();
        static PHP_FN_RE: OnceLock<Regex> = OnceLock::new();
        static GO_FN_RE: OnceLock<Regex> = OnceLock::new();
        static JAVA_FN_RE: OnceLock<Regex> = OnceLock::new();
        static KOTLIN_FN_RE: OnceLock<Regex> = OnceLock::new();

        let class_re = CLASS_RE.get_or_init(|| {
            Regex::new(
                r"^\s*(?:(?:public|private|protected|internal|open|abstract|final|sealed|data|annotation|inner|inline|value|actual|expect|static|enum)\s+)*(class|interface|object|struct|enum|trait)\s+([A-Za-z0-9_]+)",
            )
            .unwrap()
        });
        let import_re = IMPORT_RE.get_or_init(|| {
            Regex::new(r#"^\s*(?:import|include|require|using|use)\s+['"<]?([^'">;\s]+)['">]?"#)
                .unwrap()
        });
        let php_fn_re = PHP_FN_RE
            .get_or_init(|| Regex::new(r"\bfunction\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap());
        let go_fn_re = GO_FN_RE.get_or_init(|| {
            Regex::new(r"^\s*func\s+(?:\([^)]*\)\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap()
        });
        let java_fn_re = JAVA_FN_RE.get_or_init(|| {
            Regex::new(r"^\s*(?:(?:public|private|protected|internal|static|final|async|override|virtual|abstract|synchronized|sealed|partial)\s+)+(?:[\w.<>\[\]]+\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*\(")
                .unwrap()
        });
        let kotlin_fn_re = KOTLIN_FN_RE.get_or_init(|| {
            Regex::new(
                r"\bfun\s+(?:<[^>\n]+>\s+)?(?:[A-Za-z_][A-Za-z0-9_]*\.)?([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^>\n]+>)?\s*\(",
            )
            .unwrap()
        });

        let mut current_fn: Option<String> = None;
        let mut current_class: Option<String> = None;
        let mut fn_start_depth = 0i32;
        let mut class_start_depth = 0i32;
        let mut depth = 0i32;
        let mut fn_line_start = 0usize;

        for (line_idx, line) in content.lines().enumerate() {
            let line_no = line_idx + 1;

            if let Some(cap) = import_re.captures(line) {
                if let Some(src) = cap.get(1) {
                    let source = src.as_str().to_string();
                    let imported = import_basename(&source);
                    if imported.is_empty() || source.trim_end_matches(';').ends_with('*') {
                        continue;
                    }
                    result.imports.push(ParsedImport {
                        source_path: source.clone(),
                        imported_symbols: vec![imported.clone()],
                        is_default: false,
                        is_namespace: false,
                        line_number: line_no,
                    });
                    result.relationships.push(ParsedRelationship {
                        source_symbol: filename.to_string(),
                        target_symbol: imported,
                        relationship: EdgeType::Imports,
                        target_file_hint: Some(source),
                        receiver_hint: None,
                    });
                }
            }

            if let Some(cap) = class_re.captures(line) {
                if let Some(name) = cap.get(2) {
                    let name = name.as_str().to_string();
                    current_class = Some(name.clone());
                    class_start_depth = depth;
                    result.symbols.push(ParsedSymbol::new(
                        name,
                        NodeType::Class,
                        Some(line.trim().to_string()),
                        line_no..(line_no + 1),
                        true,
                    ));
                }
            }

            if let Some(fn_name) =
                match_function(line, php_fn_re, go_fn_re, java_fn_re, kotlin_fn_re)
            {
                if let Some(prev) = current_fn.take() {
                    close_function(&mut result, &prev, fn_line_start, line_no);
                }
                current_fn = Some(fn_name.clone());
                fn_start_depth = depth;
                fn_line_start = line_no;
                let exported = !line.contains("private") && !line.contains("protected");
                result.symbols.push(ParsedSymbol {
                    name: fn_name.clone(),
                    symbol_type: NodeType::Function,
                    signature: Some(line.trim().to_string()),
                    line_range: line_no..(line_no + 1),
                    docstring: None,
                    exported,
                    parent: current_class.clone(),
                    calls: Vec::new(),
                });
                extract_calls_from_line(&fn_name, line, &mut result);
                extract_type_uses_from_line(&fn_name, line, &mut result);
            } else if let Some(caller) = current_fn.as_deref() {
                extract_calls_from_line(caller, line, &mut result);
                extract_type_uses_from_line(caller, line, &mut result);
            }

            depth += brace_delta(line);

            if current_fn.is_some() && depth <= fn_start_depth && line.contains('}') {
                if let Some(prev) = current_fn.take() {
                    close_function(&mut result, &prev, fn_line_start, line_no);
                }
            }
            if current_class.is_some() && depth <= class_start_depth && line.contains('}') {
                current_class = None;
            }
        }

        if let Some(prev) = current_fn.take() {
            close_function(
                &mut result,
                &prev,
                fn_line_start,
                content.lines().count().max(1),
            );
        }

        result.exports = result
            .symbols
            .iter()
            .filter(|s| s.exported)
            .map(|s| s.name.clone())
            .collect();
        attach_calls(&mut result);
        result
    }
}

fn match_function(
    line: &str,
    php: &Regex,
    go: &Regex,
    java: &Regex,
    kotlin: &Regex,
) -> Option<String> {
    let name = kotlin
        .captures(line)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .or_else(|| {
            php.captures(line)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        })
        .or_else(|| {
            go.captures(line)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        })
        .or_else(|| {
            java.captures(line)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        })?;
    if SKIP_FN_NAMES.contains(&name.as_str()) {
        return None;
    }
    Some(name)
}

const SKIP_FN_NAMES: &[&str] = &[
    "if",
    "for",
    "while",
    "switch",
    "catch",
    "try",
    "return",
    "sizeof",
    "typeof",
    "class",
    "struct",
    "enum",
    "interface",
    "new",
    "delete",
    "using",
    "fun",
    "object",
    "constructor",
    "when",
    "package",
];

fn import_basename(source: &str) -> String {
    source
        .trim_end_matches(['*', ';'])
        .trim_end_matches('.')
        .split(['/', '\\', '.'])
        .rfind(|s| !s.is_empty() && *s != "*")
        .unwrap_or("")
        .to_string()
}

fn close_function(result: &mut AstAnalysisResult, name: &str, start: usize, end: usize) {
    if let Some(sym) = result
        .symbols
        .iter_mut()
        .rev()
        .find(|s| s.name == name && s.symbol_type == NodeType::Function)
    {
        sym.line_range = start..(end + 1);
    }
}

fn attach_calls(result: &mut AstAnalysisResult) {
    for rel in &result.relationships {
        if rel.relationship != EdgeType::Calls {
            continue;
        }
        if let Some(sym) = result
            .symbols
            .iter_mut()
            .rev()
            .find(|s| s.name == rel.source_symbol && s.symbol_type == NodeType::Function)
        {
            if !sym.calls.contains(&rel.target_symbol) {
                sym.calls.push(rel.target_symbol.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn php_functions_throw_and_catch_become_calls() {
        let exception = r#"
<?php
namespace App\Installer\Component;

final class InstallPlatformException extends \RuntimeException
{
}
"#;
        let config = r#"
<?php
namespace App\Installer\Component;

final class InstallPlatformConfig
{
    public function load(): array
    {
        throw new InstallPlatformException('missing');
    }

    public function validate(array $config): void
    {
        throw new InstallPlatformException('invalid');
    }
}
"#;
        let command = r#"
<?php
use App\Installer\Component\InstallPlatformException;

final class InstallPlatformCommand
{
    public function execute(): int
    {
        try {
            (new InstallPlatformConfig())->load();
        } catch (InstallPlatformException $e) {
            return $this->failConfig($e);
        }
        return 0;
    }

    private function failConfig(InstallPlatformException $e): int
    {
        return 1;
    }
}
"#;

        let ex = GenericParser::parse(&PathBuf::from("InstallPlatformException.php"), exception);
        assert!(ex
            .symbols
            .iter()
            .any(|s| s.name == "InstallPlatformException" && s.symbol_type == NodeType::Class));

        let cfg = GenericParser::parse(&PathBuf::from("InstallPlatformConfig.php"), config);
        assert!(cfg.symbols.iter().any(|s| s.name == "load"));
        assert!(cfg.relationships.iter().any(|r| {
            r.relationship == EdgeType::Calls
                && r.source_symbol == "load"
                && r.target_symbol == "InstallPlatformException"
        }));
        assert!(cfg.relationships.iter().any(|r| {
            r.relationship == EdgeType::Calls
                && r.source_symbol == "validate"
                && r.target_symbol == "InstallPlatformException"
        }));

        let cmd = GenericParser::parse(&PathBuf::from("InstallPlatformCommand.php"), command);
        assert!(cmd.imports.iter().any(|i| i
            .imported_symbols
            .contains(&"InstallPlatformException".into())));
        assert!(cmd.relationships.iter().any(|r| {
            r.relationship == EdgeType::Calls
                && r.source_symbol == "execute"
                && r.target_symbol == "InstallPlatformException"
        }));
        assert!(cmd.relationships.iter().any(|r| {
            r.relationship == EdgeType::Calls
                && r.source_symbol == "failConfig"
                && r.target_symbol == "InstallPlatformException"
        }));
        assert!(cmd.relationships.iter().any(|r| {
            r.relationship == EdgeType::Calls
                && r.source_symbol == "execute"
                && r.target_symbol == "load"
        }));
    }

    #[test]
    fn kotlin_fun_object_and_imports() {
        let store = r#"
package com.example.app

object SmsStore {
    fun save(body: String?) {
        persist(body)
    }

    private fun persist(body: String?) {
    }
}

data class Message(val id: Long, val body: String)
"#;
        let receiver = r#"
package com.example.app

import com.example.app.SmsStore
import android.content.Intent

class SmsReceiver {
    fun onReceive(intent: Intent) {
        val body = intent.getStringExtra("sms")
        try {
            SmsStore.save(body)
        } catch (e: SmsStoreException) {
            return
        }
    }
}

class SmsStoreException : RuntimeException()
"#;

        let store_ast = GenericParser::parse(&PathBuf::from("SmsStore.kt"), store);
        assert!(store_ast
            .symbols
            .iter()
            .any(|s| s.name == "SmsStore" && s.symbol_type == NodeType::Class));
        assert!(store_ast.symbols.iter().any(|s| s.name == "save"));
        assert!(store_ast.symbols.iter().any(|s| s.name == "persist"));
        assert!(store_ast.symbols.iter().any(|s| s.name == "Message"));
        assert!(store_ast.relationships.iter().any(|r| {
            r.relationship == EdgeType::Calls
                && r.source_symbol == "save"
                && r.target_symbol == "persist"
        }));
        assert!(
            !store_ast.symbols.iter().any(|s| s.name == "com"),
            "package line must not become a class: {:?}",
            store_ast
                .symbols
                .iter()
                .map(|s| &s.name)
                .collect::<Vec<_>>()
        );

        let recv = GenericParser::parse(&PathBuf::from("SmsReceiver.kt"), receiver);
        assert!(recv
            .imports
            .iter()
            .any(|i| i.imported_symbols.contains(&"SmsStore".into())));
        assert!(recv.symbols.iter().any(|s| s.name == "onReceive"));
        assert!(recv.relationships.iter().any(|r| {
            r.relationship == EdgeType::Calls
                && r.source_symbol == "onReceive"
                && r.target_symbol == "save"
        }));
        assert!(recv.relationships.iter().any(|r| {
            r.relationship == EdgeType::Calls
                && r.source_symbol == "onReceive"
                && r.target_symbol == "getStringExtra"
        }));
        assert!(recv.relationships.iter().any(|r| {
            r.relationship == EdgeType::Calls
                && r.source_symbol == "onReceive"
                && r.target_symbol == "SmsStoreException"
        }));
    }
}
