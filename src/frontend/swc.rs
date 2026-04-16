use std::{collections::BTreeSet, path::Path};

use anyhow::{anyhow, Result};
use swc_common::{sync::Lrc, FileName, SourceMap, Span, Spanned};
use swc_ecma_ast::{
    ArrowExpr, CallExpr, Callee, Decl, EsVersion, Expr, FnDecl, Function, Lit, MemberExpr,
    MemberProp, Module, ModuleDecl, ModuleItem, NewExpr, Pat, VarDecl, VarDeclarator,
};
use swc_ecma_parser::{lexer::Lexer, EsSyntax, Parser, StringInput, Syntax, TsSyntax};
use swc_ecma_visit::{Visit, VisitWith};

use crate::{
    config::{normalize_path, ResolvedConfig},
    discovery::RepoDiscovery,
    frontend::language_for_path,
    ids::document_id,
    model::{ArtifactDoc, WarningDoc},
    security::apply_artifact_security,
};

struct ExportedSymbol {
    name: String,
    span: Span,
    is_async: bool,
    invoke_key: Option<String>,
}

#[derive(Clone)]
struct FunctionContext {
    name: String,
}

pub fn extract_file(
    config: &ResolvedConfig,
    path: &Path,
    text: &str,
    known_hooks: &BTreeSet<String>,
    guest_binding: bool,
) -> Result<(Vec<ArtifactDoc>, Vec<WarningDoc>)> {
    let cm: Lrc<SourceMap> = Default::default();
    let file_name = FileName::Real(path.to_path_buf());
    let fm = cm.new_source_file(file_name.into(), text.to_owned());
    let syntax = syntax_for_path(path)?;
    let lexer = Lexer::new(syntax, EsVersion::Es2022, StringInput::from(&*fm), None);
    let mut parser = Parser::new_from(lexer);
    let module = parser
        .parse_module()
        .map_err(|error| anyhow!("swc parse failed for {}: {error:?}", path.display()))?;

    let exported = collect_exported_symbols(&module);
    let mut artifacts = Vec::new();
    let mut warnings = Vec::new();

    for item in &exported {
        if item.name.starts_with("use") {
            let mut doc = base_artifact(
                config,
                path,
                &cm,
                "frontend_hook_def",
                &item.name,
                item.span,
            );
            doc.display_name = Some(format!("{} hook", item.name));
            doc.tags = vec!["custom hook".to_owned()];
            doc.data.insert(
                "hook_kind".to_owned(),
                serde_json::Value::String(classify_hook_kind(text).to_owned()),
            );
            doc.data.insert(
                "requires_cleanup".to_owned(),
                serde_json::Value::Bool(text.contains("listen(") || text.contains("once(")),
            );
            doc.data.insert(
                "cleanup_present".to_owned(),
                serde_json::Value::Bool(text.contains("return () =>") || text.contains("unlisten")),
            );
            apply_artifact_security(&mut doc);
            artifacts.push(doc);
        } else if item
            .name
            .chars()
            .next()
            .map(|ch| ch.is_uppercase())
            .unwrap_or(false)
        {
            let mut doc = base_artifact(
                config,
                path,
                &cm,
                "frontend_component",
                &item.name,
                item.span,
            );
            doc.display_name = Some(format!("{} component", item.name));
            doc.tags = vec!["component".to_owned()];
            doc.data.insert(
                "component".to_owned(),
                serde_json::Value::String(item.name.clone()),
            );
            apply_artifact_security(&mut doc);
            artifacts.push(doc);
        }

        if guest_binding && item.is_async {
            if let Some(invoke_key) = item.invoke_key.as_ref() {
                let mut doc = base_artifact(
                    config,
                    path,
                    &cm,
                    "tauri_plugin_binding",
                    &item.name,
                    item.span,
                );
                doc.display_name = Some(format!("{} plugin binding", item.name));
                doc.data.insert(
                    "plugin_export".to_owned(),
                    serde_json::Value::String(item.name.clone()),
                );
                doc.data.insert(
                    "invoke_key".to_owned(),
                    serde_json::Value::String(invoke_key.to_owned()),
                );
                if let Some(plugin_name) = invoke_key
                    .strip_prefix("plugin:")
                    .and_then(|value| value.split('|').next())
                {
                    doc.data.insert(
                        "plugin_name".to_owned(),
                        serde_json::Value::String(plugin_name.to_owned()),
                    );
                }
                apply_artifact_security(&mut doc);
                artifacts.push(doc);
            }
        }
    }

    let mut visitor = SwcCollector {
        config,
        path,
        cm,
        known_hooks,
        artifacts: Vec::new(),
        warnings: Vec::new(),
        function_stack: Vec::new(),
    };
    module.visit_with(&mut visitor);
    artifacts.extend(visitor.artifacts);
    warnings.extend(visitor.warnings);

    Ok((artifacts, warnings))
}

pub fn discover_hook_names(discovery: &RepoDiscovery) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for path in &discovery.frontend_files {
        let text = std::fs::read_to_string(path)?;
        let cm: Lrc<SourceMap> = Default::default();
        let file_name = FileName::Real(path.to_path_buf());
        let fm = cm.new_source_file(file_name.into(), text);
        let syntax = syntax_for_path(path)?;
        let lexer = Lexer::new(syntax, EsVersion::Es2022, StringInput::from(&*fm), None);
        let mut parser = Parser::new_from(lexer);
        if let Ok(module) = parser.parse_module() {
            for export in collect_exported_symbols(&module) {
                if export.name.starts_with("use") {
                    names.insert(export.name);
                }
            }
        }
    }
    Ok(names)
}

fn syntax_for_path(path: &Path) -> Result<Syntax> {
    let extension = path
        .extension()
        .and_then(|item| item.to_str())
        .unwrap_or("");
    Ok(match extension {
        "ts" => Syntax::Typescript(TsSyntax {
            tsx: false,
            decorators: true,
            ..Default::default()
        }),
        "tsx" => Syntax::Typescript(TsSyntax {
            tsx: true,
            decorators: true,
            ..Default::default()
        }),
        "jsx" => Syntax::Es(EsSyntax {
            jsx: true,
            ..Default::default()
        }),
        "js" | "mjs" | "cjs" => Syntax::Es(EsSyntax {
            jsx: false,
            ..Default::default()
        }),
        other => return Err(anyhow!("unsupported frontend extension {other}")),
    })
}

fn base_artifact(
    config: &ResolvedConfig,
    path: &Path,
    cm: &Lrc<SourceMap>,
    kind: &str,
    name: &str,
    span: Span,
) -> ArtifactDoc {
    let source_path = normalize_path(&config.root, path);
    let start = cm.lookup_char_pos(span.lo());
    let end = cm.lookup_char_pos(span.hi());
    ArtifactDoc {
        id: document_id(
            &config.repo,
            kind,
            Some(&source_path),
            Some(start.line as u32),
            Some(name),
        ),
        repo: config.repo.clone(),
        kind: kind.to_owned(),
        side: Some("frontend".to_owned()),
        language: language_for_path(path),
        name: Some(name.to_owned()),
        display_name: Some(name.to_owned()),
        source_path: Some(source_path),
        line_start: Some(start.line as u32),
        line_end: Some(end.line as u32),
        column_start: Some(start.col_display as u32),
        column_end: Some(end.col_display as u32),
        package_name: None,
        comments: Vec::new(),
        tags: Vec::new(),
        related_symbols: Vec::new(),
        related_tests: Vec::new(),
        risk_level: "low".to_owned(),
        risk_reasons: Vec::new(),
        contains_phi: false,
        has_related_tests: false,
        updated_at: chrono::Utc::now().to_rfc3339(),
        data: {
            let mut data = serde_json::Map::new();
            data.insert(
                "source_map_backend".to_owned(),
                serde_json::Value::String("swc".to_owned()),
            );
            data
        },
    }
}

fn collect_exported_symbols(module: &Module) -> Vec<ExportedSymbol> {
    let mut exports = Vec::new();
    for item in &module.body {
        if let ModuleItem::ModuleDecl(module_decl) = item {
            match module_decl {
                ModuleDecl::ExportDecl(export_decl) => match &export_decl.decl {
                    Decl::Fn(fn_decl) => exports.push(exported_fn_decl(fn_decl)),
                    Decl::Var(var_decl) => exports.extend(exported_var_decl(var_decl)),
                    _ => {}
                },
                ModuleDecl::ExportDefaultDecl(default_decl) => {
                    if let swc_ecma_ast::DefaultDecl::Fn(fn_expr) = &default_decl.decl {
                        if let Some(ident) = &fn_expr.ident {
                            exports.push(ExportedSymbol {
                                name: ident.sym.to_string(),
                                span: fn_expr.function.span,
                                is_async: fn_expr.function.is_async,
                                invoke_key: fn_expr
                                    .function
                                    .body
                                    .as_ref()
                                    .and_then(find_invoke_key_in_block_stmt),
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }
    exports
}

fn exported_fn_decl(fn_decl: &FnDecl) -> ExportedSymbol {
    ExportedSymbol {
        name: fn_decl.ident.sym.to_string(),
        span: fn_decl.function.span,
        is_async: fn_decl.function.is_async,
        invoke_key: fn_decl
            .function
            .body
            .as_ref()
            .and_then(find_invoke_key_in_block_stmt),
    }
}

fn exported_var_decl(var_decl: &VarDecl) -> Vec<ExportedSymbol> {
    var_decl
        .decls
        .iter()
        .filter_map(exported_var_symbol)
        .collect()
}

fn exported_var_symbol(decl: &VarDeclarator) -> Option<ExportedSymbol> {
    let Pat::Ident(ident) = &decl.name else {
        return None;
    };
    let name = ident.id.sym.to_string();
    match decl.init.as_deref() {
        Some(Expr::Arrow(arrow)) => Some(ExportedSymbol {
            name,
            span: arrow.span,
            is_async: arrow.is_async,
            invoke_key: find_invoke_key_in_arrow(arrow),
        }),
        Some(Expr::Fn(fn_expr)) => Some(ExportedSymbol {
            name,
            span: fn_expr.function.span,
            is_async: fn_expr.function.is_async,
            invoke_key: fn_expr
                .function
                .body
                .as_ref()
                .and_then(find_invoke_key_in_block_stmt),
        }),
        _ => None,
    }
}

fn find_invoke_key_in_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Call(call) => literal_arg(&call.args).or_else(|| {
            call.args
                .iter()
                .find_map(|arg| find_invoke_key_in_expr(arg.expr.as_ref()))
        }),
        Expr::Arrow(ArrowExpr { body, .. }) => match body.as_ref() {
            swc_ecma_ast::BlockStmtOrExpr::Expr(inner) => find_invoke_key_in_expr(inner.as_ref()),
            swc_ecma_ast::BlockStmtOrExpr::BlockStmt(block) => {
                block.stmts.iter().find_map(find_invoke_key_in_stmt)
            }
        },
        Expr::Await(await_expr) => find_invoke_key_in_expr(await_expr.arg.as_ref()),
        Expr::Paren(paren) => find_invoke_key_in_expr(paren.expr.as_ref()),
        _ => None,
    }
}

fn find_invoke_key_in_arrow(arrow: &ArrowExpr) -> Option<String> {
    match arrow.body.as_ref() {
        swc_ecma_ast::BlockStmtOrExpr::Expr(expr) => find_invoke_key_in_expr(expr.as_ref()),
        swc_ecma_ast::BlockStmtOrExpr::BlockStmt(block) => find_invoke_key_in_block_stmt(block),
    }
}

fn find_invoke_key_in_block_stmt(block: &swc_ecma_ast::BlockStmt) -> Option<String> {
    block.stmts.iter().find_map(find_invoke_key_in_stmt)
}

fn find_invoke_key_in_stmt(stmt: &swc_ecma_ast::Stmt) -> Option<String> {
    match stmt {
        swc_ecma_ast::Stmt::Expr(expr_stmt) => find_invoke_key_in_expr(expr_stmt.expr.as_ref()),
        swc_ecma_ast::Stmt::Return(return_stmt) => return_stmt
            .arg
            .as_ref()
            .and_then(|expr| find_invoke_key_in_expr(expr.as_ref())),
        swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Var(var_decl)) => {
            var_decl.decls.iter().find_map(|decl| {
                decl.init
                    .as_ref()
                    .and_then(|expr| find_invoke_key_in_expr(expr))
            })
        }
        _ => None,
    }
}

fn classify_hook_kind(text: &str) -> &'static str {
    if text.contains("new Channel") || text.contains("Channel<") {
        "channel_stream"
    } else if text.contains("listen(") || text.contains("once(") {
        "event_subscription"
    } else if text.contains("invoke(") || text.contains("__TAURI__.invoke(") {
        "invoke_once"
    } else {
        "unknown"
    }
}

struct SwcCollector<'a> {
    config: &'a ResolvedConfig,
    path: &'a Path,
    cm: Lrc<SourceMap>,
    known_hooks: &'a BTreeSet<String>,
    artifacts: Vec<ArtifactDoc>,
    warnings: Vec<WarningDoc>,
    function_stack: Vec<FunctionContext>,
}

impl SwcCollector<'_> {
    fn current_context(&self) -> Option<&FunctionContext> {
        self.function_stack.last()
    }

    fn push_named_function(&mut self, name: String) {
        self.function_stack.push(FunctionContext { name });
    }

    fn pop_named_function(&mut self) {
        let _ = self.function_stack.pop();
    }

    fn add_hook_use(&mut self, name: &str, span: Span) {
        let mut doc = base_artifact(
            self.config,
            self.path,
            &self.cm,
            "frontend_hook_use",
            name,
            span,
        );
        if let Some(context) = self.current_context() {
            doc.data.insert(
                "component".to_owned(),
                serde_json::Value::String(context.name.clone()),
            );
            doc.display_name = Some(format!("{} uses {}", context.name, name));
        }
        doc.data.insert(
            "hook_kind".to_owned(),
            serde_json::Value::String("unknown".to_owned()),
        );
        doc.data.insert(
            "hook_def_name".to_owned(),
            serde_json::Value::String(name.to_owned()),
        );
        apply_artifact_security(&mut doc);
        self.artifacts.push(doc);
    }

    fn add_invoke(&mut self, invoke_key: &str, span: Span) {
        let name = invoke_key
            .split('|')
            .next_back()
            .unwrap_or(invoke_key)
            .split(':')
            .next_back()
            .unwrap_or(invoke_key);
        let mut doc = base_artifact(self.config, self.path, &self.cm, "tauri_invoke", name, span);
        doc.display_name = Some(format!("invoke {}", invoke_key));
        doc.tags = vec!["tauri invoke".to_owned()];
        doc.data.insert(
            "invoke_key".to_owned(),
            serde_json::Value::String(invoke_key.to_owned()),
        );
        doc.data.insert(
            "command_name".to_owned(),
            serde_json::Value::String(name.to_owned()),
        );
        if let Some(context) = self.current_context() {
            doc.data.insert(
                "nearest_symbol".to_owned(),
                serde_json::Value::String(context.name.clone()),
            );
        }
        if let Some(plugin_name) = invoke_key
            .strip_prefix("plugin:")
            .and_then(|value| value.split('|').next())
        {
            doc.data.insert(
                "plugin_name".to_owned(),
                serde_json::Value::String(plugin_name.to_owned()),
            );
        }
        apply_artifact_security(&mut doc);
        self.artifacts.push(doc);
    }

    fn add_event(&mut self, kind: &str, event_name: &str, span: Span) {
        let mut doc = base_artifact(self.config, self.path, &self.cm, kind, event_name, span);
        doc.data.insert(
            "event_name".to_owned(),
            serde_json::Value::String(event_name.to_owned()),
        );
        doc.tags = vec!["event".to_owned()];
        apply_artifact_security(&mut doc);
        self.artifacts.push(doc);
    }

    fn add_channel(&mut self, channel_name: &str, span: Span) {
        let mut doc = base_artifact(
            self.config,
            self.path,
            &self.cm,
            "tauri_channel",
            channel_name,
            span,
        );
        doc.display_name = Some(format!("Channel {}", channel_name));
        doc.data.insert(
            "channel_name".to_owned(),
            serde_json::Value::String(channel_name.to_owned()),
        );
        apply_artifact_security(&mut doc);
        self.artifacts.push(doc);
    }

    fn add_dynamic_invoke_warning(&mut self, variable_name: &str, span: Span) {
        let source_path = normalize_path(&self.config.root, self.path);
        let start = self.cm.lookup_char_pos(span.lo());
        self.warnings.push(WarningDoc {
            id: document_id(
                &self.config.repo,
                "warning",
                Some(&source_path),
                Some(start.line as u32),
                Some("dynamic_invoke"),
            ),
            repo: self.config.repo.clone(),
            kind: "warning".to_owned(),
            warning_type: "dynamic_invoke".to_owned(),
            severity: "warning".to_owned(),
            message: format!(
                "Cannot statically resolve Tauri command name from {}",
                variable_name
            ),
            source_path: Some(source_path),
            line_start: Some(start.line as u32),
            related_id: None,
            risk_level: "medium".to_owned(),
            remediation: None,
            updated_at: chrono::Utc::now().to_rfc3339(),
        });
    }
}

impl Visit for SwcCollector<'_> {
    fn visit_fn_decl(&mut self, fn_decl: &FnDecl) {
        self.push_named_function(fn_decl.ident.sym.to_string());
        fn_decl.function.visit_with(self);
        self.pop_named_function();
    }

    fn visit_function(&mut self, function: &Function) {
        function.visit_children_with(self);
    }

    fn visit_var_declarator(&mut self, declarator: &VarDeclarator) {
        if let Pat::Ident(ident) = &declarator.name {
            if let Some(init) = &declarator.init {
                match init.as_ref() {
                    Expr::Arrow(arrow) => {
                        self.push_named_function(ident.id.sym.to_string());
                        arrow.visit_children_with(self);
                        self.pop_named_function();
                        return;
                    }
                    Expr::Fn(fn_expr) => {
                        self.push_named_function(ident.id.sym.to_string());
                        fn_expr.function.visit_children_with(self);
                        self.pop_named_function();
                        return;
                    }
                    Expr::New(new_expr) => {
                        if is_channel_constructor(new_expr) {
                            self.add_channel(ident.id.sym.as_ref(), declarator.span());
                        }
                        return;
                    }
                    _ => {}
                }
            }
        }
        declarator.visit_children_with(self);
    }

    fn visit_call_expr(&mut self, call: &CallExpr) {
        if let Some(name) = hook_call_name(call) {
            if self.known_hooks.contains(name) {
                self.add_hook_use(name, call.span);
            }
        }

        if let Some(invoke_key) = invoke_key_from_call(call) {
            self.add_invoke(&invoke_key, call.span);
        } else if let Some(dynamic_name) = dynamic_invoke_name(call) {
            self.add_dynamic_invoke_warning(&dynamic_name, call.span);
        }

        if let Some((kind, event_name)) = event_from_call(call) {
            self.add_event(kind, &event_name, call.span);
        }

        call.visit_children_with(self);
    }
}

fn hook_call_name(call: &CallExpr) -> Option<&str> {
    match &call.callee {
        Callee::Expr(expr) => match expr.as_ref() {
            Expr::Ident(ident) if ident.sym.starts_with("use") => Some(ident.sym.as_ref()),
            _ => None,
        },
        _ => None,
    }
}

fn invoke_key_from_call(call: &CallExpr) -> Option<String> {
    if !matches_invoke_callee(&call.callee) {
        return None;
    }
    literal_arg(&call.args)
}

fn dynamic_invoke_name(call: &CallExpr) -> Option<String> {
    if !matches_invoke_callee(&call.callee) {
        return None;
    }
    let arg = call.args.first()?.expr.as_ref();
    match arg {
        Expr::Ident(ident) => Some(ident.sym.to_string()),
        _ => None,
    }
}

fn event_from_call(call: &CallExpr) -> Option<(&'static str, String)> {
    let method = match &call.callee {
        Callee::Expr(expr) => match expr.as_ref() {
            Expr::Ident(ident) => ident.sym.to_string(),
            Expr::Member(member) => member_property_name(member)?,
            _ => return None,
        },
        _ => return None,
    };
    let kind = match method.as_str() {
        "emit" => "tauri_event_emit",
        "listen" | "once" => "tauri_event_listener",
        _ => return None,
    };
    literal_arg(&call.args).map(|value| (kind, value))
}

fn literal_arg(args: &[swc_ecma_ast::ExprOrSpread]) -> Option<String> {
    let arg = args.first()?.expr.as_ref();
    match arg {
        Expr::Lit(Lit::Str(str_lit)) => Some(str_lit.value.to_string_lossy().to_string()),
        _ => None,
    }
}

fn matches_invoke_callee(callee: &Callee) -> bool {
    match callee {
        Callee::Expr(expr) => match expr.as_ref() {
            Expr::Ident(ident) => ident.sym == *"invoke",
            Expr::Member(member) => member_chain_ends_with_invoke(member),
            _ => false,
        },
        _ => false,
    }
}

fn member_chain_ends_with_invoke(member: &MemberExpr) -> bool {
    if member_property_name(member).as_deref() != Some("invoke") {
        return false;
    }
    true
}

fn member_property_name(member: &MemberExpr) -> Option<String> {
    match &member.prop {
        MemberProp::Ident(ident) => Some(ident.sym.to_string()),
        MemberProp::PrivateName(private) => Some(private.name.to_string()),
        MemberProp::Computed(computed) => match computed.expr.as_ref() {
            Expr::Lit(Lit::Str(str_lit)) => Some(str_lit.value.to_string_lossy().to_string()),
            _ => None,
        },
    }
}

fn is_channel_constructor(new_expr: &NewExpr) -> bool {
    match new_expr.callee.as_ref() {
        Expr::Ident(ident) => ident.sym == *"Channel",
        _ => false,
    }
}
