use crate::core::indicators::compiler::ast::{AstCall, AstFnDecl, AstProgram, AstStatement};
use crate::core::indicators::{
    IrBinaryOp, IrCall, IrCallArg, IrCallKind, IrExpr, IrSeriesField, OpCode,
};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct LoweredIr {
    pub opcodes: Vec<OpCode>,
    pub calls: Vec<IrCall>,
}

pub fn lower_to_ir(program: &AstProgram) -> LoweredIr {
    let mut opcodes = Vec::new();
    let mut calls = Vec::new();
    let mut functions = HashMap::<String, AstFnDecl>::new();

    for statement in &program.statements {
        if let AstStatement::FnDecl(function) = statement {
            functions.insert(function.name.to_ascii_lowercase(), function.clone());
        }
    }

    let mut call_stack = Vec::<String>::new();
    for statement in &program.statements {
        lower_statement(
            statement,
            None,
            &functions,
            &mut call_stack,
            &mut opcodes,
            &mut calls,
        );
    }

    if opcodes.is_empty() {
        opcodes.push(OpCode::Nop);
    }
    opcodes.push(OpCode::Halt);

    LoweredIr { opcodes, calls }
}

fn lower_statement(
    statement: &AstStatement,
    guard: Option<&IrExpr>,
    functions: &HashMap<String, AstFnDecl>,
    call_stack: &mut Vec<String>,
    opcodes: &mut Vec<OpCode>,
    calls: &mut Vec<IrCall>,
) {
    match statement {
        AstStatement::Call(call) => {
            lower_call(call, guard, functions, call_stack, opcodes, calls);
        }
        AstStatement::For(for_loop) => {
            opcodes.push(OpCode::BranchIfTrue);
            let iterations = for_loop
                .end
                .saturating_sub(for_loop.start)
                .saturating_add(1)
                .min(1024);
            for _ in 0..iterations {
                for body_stmt in &for_loop.body {
                    lower_statement(body_stmt, guard, functions, call_stack, opcodes, calls);
                }
            }
        }
        AstStatement::If(branch) => {
            opcodes.push(OpCode::BranchIfTrue);
            let Some(condition) = parse_expression(&branch.condition) else {
                return;
            };

            let then_guard = combine_guards(guard, Some(condition.clone()));
            let else_guard = combine_guards(
                guard,
                Some(IrExpr::UnaryNot(Box::new(condition.clone()))),
            );

            for body_stmt in &branch.then_branch {
                lower_statement(
                    body_stmt,
                    then_guard.as_ref(),
                    functions,
                    call_stack,
                    opcodes,
                    calls,
                );
            }

            if !branch.else_branch.is_empty() {
                opcodes.push(OpCode::BranchIfFalse);
                for body_stmt in &branch.else_branch {
                    lower_statement(
                        body_stmt,
                        else_guard.as_ref(),
                        functions,
                        call_stack,
                        opcodes,
                        calls,
                    );
                }
            }
        }
        AstStatement::VarDecl(decl) => {
            let kind = if decl.is_persistent {
                IrCallKind::StateVarDecl
            } else {
                IrCallKind::StateLetDecl
            };
            let value_expr = decl
                .value
                .as_ref()
                .and_then(|raw| parse_expression(raw))
                .unwrap_or(IrExpr::Na);
            calls.push(IrCall {
                kind,
                args: vec![IrCallArg::Text(decl.name.clone()), IrCallArg::Expr(value_expr)],
                guard: guard.cloned(),
                declaration_order: decl.line.saturating_sub(1) as u32,
            });
            if decl.value.is_some() {
                opcodes.push(OpCode::StoreSeries);
            } else {
                opcodes.push(OpCode::Nop);
            }
        }
        AstStatement::Assign(assign) => {
            let value_expr = parse_expression(&assign.value).unwrap_or(IrExpr::Na);
            calls.push(IrCall {
                kind: IrCallKind::StateAssign,
                args: vec![
                    IrCallArg::Text(assign.name.clone()),
                    IrCallArg::Expr(value_expr),
                ],
                guard: guard.cloned(),
                declaration_order: assign.line.saturating_sub(1) as u32,
            });
            if parse_expression(&assign.value).is_some() {
                opcodes.push(OpCode::StoreSeries);
            } else {
                opcodes.push(OpCode::Nop);
            }
        }
        AstStatement::FnDecl(_) => {}
        AstStatement::Return(_) => {
            opcodes.push(OpCode::BranchIfFalse);
        }
    }
}

fn combine_guards(parent: Option<&IrExpr>, child: Option<IrExpr>) -> Option<IrExpr> {
    match (parent, child) {
        (None, None) => None,
        (Some(parent_expr), None) => Some(parent_expr.clone()),
        (None, Some(child_expr)) => Some(child_expr),
        (Some(parent_expr), Some(child_expr)) => Some(IrExpr::Binary {
            lhs: Box::new(parent_expr.clone()),
            op: IrBinaryOp::And,
            rhs: Box::new(child_expr),
        }),
    }
}

fn lower_call(
    call: &AstCall,
    guard: Option<&IrExpr>,
    functions: &HashMap<String, AstFnDecl>,
    call_stack: &mut Vec<String>,
    opcodes: &mut Vec<OpCode>,
    calls: &mut Vec<IrCall>,
) {
    if let Some((kind, opcode)) = map_call_kind(&call.function) {
        calls.push(IrCall {
            kind: kind.clone(),
            args: lower_call_args(&kind, &call.args),
            guard: guard.cloned(),
            declaration_order: call.line.saturating_sub(1) as u32,
        });
        opcodes.push(opcode);
        return;
    }

    let function_name = call.function.trim().to_ascii_lowercase();
    let Some(function) = functions.get(&function_name) else {
        return;
    };
    if call_stack.contains(&function_name) {
        return;
    }

    call_stack.push(function_name.clone());
    for statement in &function.body {
        lower_statement(statement, guard, functions, call_stack, opcodes, calls);
    }
    call_stack.pop();
}

fn map_call_kind(function: &str) -> Option<(IrCallKind, OpCode)> {
    let normalized = function.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "plotcandle" | "viz.plotcandle" => Some((IrCallKind::PlotCandle, OpCode::EmitPlotCandle)),
        "plotbar" | "viz.plotbar" => Some((IrCallKind::PlotBar, OpCode::EmitPlotBar)),
        "plothistogram" | "plot_histogram" | "viz.plothistogram" | "viz.plot_histogram" => {
            Some((IrCallKind::PlotHistogram, OpCode::CallBuiltin))
        }
        "plotarea" | "viz.plotarea" => Some((IrCallKind::PlotArea, OpCode::EmitPlotArea)),
        "fillbetween" | "fill_between" | "viz.fillbetween" | "viz.fill_between" => {
            Some((IrCallKind::FillBetween, OpCode::EmitFillBetween))
        }
        "plotshape" | "viz.plotshape" => Some((IrCallKind::PlotShape, OpCode::EmitPlotShape)),
        "plot" | "viz.plot" => Some((IrCallKind::PlotLine, OpCode::EmitPlotLine)),
        "box.new" | "obj.new_box" => Some((IrCallKind::ObjBoxNew, OpCode::EmitDrawBox)),
        "box.set" | "obj.set_box" => Some((IrCallKind::ObjBoxSet, OpCode::EmitDrawBox)),
        "box.delete" | "obj.delete_box" => Some((IrCallKind::ObjBoxDelete, OpCode::EmitDrawBox)),
        "label.new" | "obj.new_label" => Some((IrCallKind::ObjLabelNew, OpCode::EmitDrawLabel)),
        "label.set" | "obj.set_label" => Some((IrCallKind::ObjLabelSet, OpCode::EmitDrawLabel)),
        "label.delete" | "obj.delete_label" => {
            Some((IrCallKind::ObjLabelDelete, OpCode::EmitDrawLabel))
        }
        "polyline.new" | "obj.new_polyline" => {
            Some((IrCallKind::ObjPolylineNew, OpCode::EmitDrawPolyline))
        }
        "polyline.set" | "obj.set_polyline" => {
            Some((IrCallKind::ObjPolylineSet, OpCode::EmitDrawPolyline))
        }
        "polyline.delete" | "obj.delete_polyline" => {
            Some((IrCallKind::ObjPolylineDelete, OpCode::EmitDrawPolyline))
        }
        "obj.delete" | "obj.remove" => Some((IrCallKind::ObjDelete, OpCode::CallBuiltin)),
        "req.series" => Some((IrCallKind::RequestSeries, OpCode::RequestSeries)),
        _ => None,
    }
}

fn lower_call_args(kind: &IrCallKind, args: &[String]) -> Vec<IrCallArg> {
    args.iter()
        .map(|arg| lower_call_arg(kind, arg))
        .collect::<Vec<_>>()
}

fn lower_call_arg(kind: &IrCallKind, arg: &str) -> IrCallArg {
    let Some((key, value)) = split_named_arg(arg) else {
        return lower_positional_arg(kind, arg);
    };
    let name = key.trim().to_string();
    let trimmed_value = value.trim();
    if let Some(expr) = parse_expression(trimmed_value) {
        return IrCallArg::NamedExpr { name, value: expr };
    }
    IrCallArg::NamedText {
        name,
        value: parse_text_argument(trimmed_value),
    }
}

fn split_named_arg(raw: &str) -> Option<(&str, &str)> {
    let bytes = raw.as_bytes();
    let mut depth_paren = 0usize;
    let mut depth_bracket = 0usize;
    let mut depth_brace = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in bytes.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if *ch == b'\\' {
                escaped = true;
                continue;
            }
            if *ch == b'"' {
                in_string = false;
            }
            continue;
        }

        match *ch {
            b'"' => in_string = true,
            b'(' => depth_paren = depth_paren.saturating_add(1),
            b')' => depth_paren = depth_paren.saturating_sub(1),
            b'[' => depth_bracket = depth_bracket.saturating_add(1),
            b']' => depth_bracket = depth_bracket.saturating_sub(1),
            b'{' => depth_brace = depth_brace.saturating_add(1),
            b'}' => depth_brace = depth_brace.saturating_sub(1),
            b'=' if depth_paren == 0 && depth_bracket == 0 && depth_brace == 0 => {
                let prev = idx.checked_sub(1).and_then(|i| bytes.get(i)).copied();
                let next = bytes.get(idx + 1).copied();
                if matches!(prev, Some(b'!' | b'<' | b'>' | b'=')) || next == Some(b'=') {
                    continue;
                }
                return Some((&raw[..idx], &raw[idx + 1..]));
            }
            _ => {}
        }
    }
    None
}

fn lower_positional_arg(kind: &IrCallKind, arg: &str) -> IrCallArg {
    if matches!(kind, IrCallKind::FillBetween) {
        return IrCallArg::Text(parse_text_argument(arg));
    }
    if let Some(expr) = parse_expression(arg) {
        return IrCallArg::Expr(expr);
    }
    IrCallArg::Text(parse_text_argument(arg))
}

fn parse_text_argument(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_expression(input: &str) -> Option<IrExpr> {
    let mut parser = ExprParser::new(input);
    let expr = parser.parse_expression().ok()?;
    parser.skip_ws();
    if parser.is_done() {
        Some(expr)
    } else {
        None
    }
}

struct ExprParser<'a> {
    source: &'a str,
    pos: usize,
}

impl<'a> ExprParser<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, pos: 0 }
    }

    fn is_done(&self) -> bool {
        self.pos >= self.source.len()
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_whitespace() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    fn take_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn starts_with(&self, value: &str) -> bool {
        self.source[self.pos..].starts_with(value)
    }

    fn consume(&mut self, value: &str) -> bool {
        if self.starts_with(value) {
            self.pos += value.len();
            true
        } else {
            false
        }
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        if !self.starts_with(keyword) {
            return false;
        }
        let end = self.pos + keyword.len();
        let boundary = self
            .source
            .get(end..)
            .and_then(|tail| tail.chars().next())
            .map(|ch| !is_ident_part(ch))
            .unwrap_or(true);
        if !boundary {
            return false;
        }
        self.pos = end;
        true
    }

    fn parse_expression(&mut self) -> Result<IrExpr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<IrExpr, String> {
        let mut expr = self.parse_and()?;
        loop {
            self.skip_ws();
            if self.consume("||") || self.consume_keyword("or") {
                let rhs = self.parse_and()?;
                expr = IrExpr::Binary {
                    lhs: Box::new(expr),
                    op: IrBinaryOp::Or,
                    rhs: Box::new(rhs),
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<IrExpr, String> {
        let mut expr = self.parse_equality()?;
        loop {
            self.skip_ws();
            if self.consume("&&") || self.consume_keyword("and") {
                let rhs = self.parse_equality()?;
                expr = IrExpr::Binary {
                    lhs: Box::new(expr),
                    op: IrBinaryOp::And,
                    rhs: Box::new(rhs),
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<IrExpr, String> {
        let mut expr = self.parse_compare()?;
        loop {
            self.skip_ws();
            let op = if self.consume("==") {
                Some(IrBinaryOp::Eq)
            } else if self.consume("!=") {
                Some(IrBinaryOp::Neq)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_compare()?;
            expr = IrExpr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_compare(&mut self) -> Result<IrExpr, String> {
        let mut expr = self.parse_add_sub()?;
        loop {
            self.skip_ws();
            let op = if self.consume(">=") {
                Some(IrBinaryOp::Gte)
            } else if self.consume("<=") {
                Some(IrBinaryOp::Lte)
            } else if self.consume(">") {
                Some(IrBinaryOp::Gt)
            } else if self.consume("<") {
                Some(IrBinaryOp::Lt)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_add_sub()?;
            expr = IrExpr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_add_sub(&mut self) -> Result<IrExpr, String> {
        let mut expr = self.parse_mul_div()?;
        loop {
            self.skip_ws();
            let op = if self.consume("+") {
                Some(IrBinaryOp::Add)
            } else if self.consume("-") {
                Some(IrBinaryOp::Sub)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_mul_div()?;
            expr = IrExpr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_mul_div(&mut self) -> Result<IrExpr, String> {
        let mut expr = self.parse_unary()?;
        loop {
            self.skip_ws();
            let op = if self.consume("*") {
                Some(IrBinaryOp::Mul)
            } else if self.consume("/") {
                Some(IrBinaryOp::Div)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_unary()?;
            expr = IrExpr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<IrExpr, String> {
        self.skip_ws();
        if self.consume("!") || self.consume_keyword("not") {
            let inner = self.parse_unary()?;
            return Ok(IrExpr::UnaryNot(Box::new(inner)));
        }
        if self.consume("-") {
            let inner = self.parse_unary()?;
            return Ok(IrExpr::UnaryNeg(Box::new(inner)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<IrExpr, String> {
        self.skip_ws();
        match self.peek_char() {
            Some('(') => {
                self.take_char();
                let expr = self.parse_expression()?;
                self.skip_ws();
                if !matches!(self.take_char(), Some(')')) {
                    return Err("expected ')'".to_string());
                }
                Ok(expr)
            }
            Some(ch) if ch.is_ascii_digit() || ch == '.' => self.parse_number(),
            Some(ch) if is_ident_start(ch) => self.parse_identifier_expr(),
            _ => Err("unexpected token in expression".to_string()),
        }
    }

    fn parse_number(&mut self) -> Result<IrExpr, String> {
        let start = self.pos;

        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                self.take_char();
            } else {
                break;
            }
        }
        if matches!(self.peek_char(), Some('.')) {
            self.take_char();
            while let Some(ch) = self.peek_char() {
                if ch.is_ascii_digit() {
                    self.take_char();
                } else {
                    break;
                }
            }
        }
        if matches!(self.peek_char(), Some('e' | 'E')) {
            self.take_char();
            if matches!(self.peek_char(), Some('+' | '-')) {
                self.take_char();
            }
            let mut has_exp_digits = false;
            while let Some(ch) = self.peek_char() {
                if ch.is_ascii_digit() {
                    has_exp_digits = true;
                    self.take_char();
                } else {
                    break;
                }
            }
            if !has_exp_digits {
                return Err("invalid exponent".to_string());
            }
        }

        let raw = &self.source[start..self.pos];
        let value = raw
            .parse::<f64>()
            .map_err(|_| "invalid number".to_string())?;
        Ok(IrExpr::Number(value))
    }

    fn parse_identifier_expr(&mut self) -> Result<IrExpr, String> {
        let ident = self.parse_identifier();
        if ident.eq_ignore_ascii_case("na") {
            return Ok(IrExpr::Na);
        }
        if ident.eq_ignore_ascii_case("true") {
            return Ok(IrExpr::Bool(true));
        }
        if ident.eq_ignore_ascii_case("false") {
            return Ok(IrExpr::Bool(false));
        }

        self.skip_ws();
        if matches!(self.peek_char(), Some('(')) {
            let args = self.parse_call_args()?;
            return self.parse_call_expr(&ident, &args);
        }

        let Some(field) = map_series_field(&ident) else {
            return Ok(IrExpr::Var(ident));
        };

        self.skip_ws();
        let index = if matches!(self.peek_char(), Some('[')) {
            self.take_char();
            let expr = self.parse_expression()?;
            self.skip_ws();
            if !matches!(self.take_char(), Some(']')) {
                return Err("expected ']'".to_string());
            }
            Some(Box::new(expr))
        } else {
            None
        };

        Ok(IrExpr::Series { field, index })
    }

    fn parse_call_args(&mut self) -> Result<Vec<String>, String> {
        if !matches!(self.take_char(), Some('(')) {
            return Err("expected '('".to_string());
        }
        let args_start = self.pos;
        let mut depth = 1usize;
        let mut in_string = false;
        let mut escaped = false;
        while let Some(ch) = self.take_char() {
            if ch == '\\' && in_string {
                escaped = !escaped;
                continue;
            }
            if ch == '"' && !escaped {
                in_string = !in_string;
                continue;
            }
            escaped = false;
            if in_string {
                continue;
            }
            if ch == '(' {
                depth = depth.saturating_add(1);
            } else if ch == ')' {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let args_end = self.pos.saturating_sub(1);
                    return Ok(split_top_level_args(&self.source[args_start..args_end]));
                }
            }
        }
        Err("unterminated call arguments".to_string())
    }

    fn parse_call_expr(&mut self, ident: &str, args: &[String]) -> Result<IrExpr, String> {
        if ident != "req.series" {
            return Err(format!("unsupported function '{}'", ident));
        }

        if args.len() < 3 {
            return Err("req.series requires (symbol, timeframe, field[, mode])".to_string());
        }
        let mode = args
            .get(3)
            .map(|it| parse_text_argument(it))
            .unwrap_or_else(|| "confirmed".to_string());
        let mut expr = IrExpr::ReqSeries {
            symbol: parse_text_argument(&args[0]),
            timeframe: parse_text_argument(&args[1]),
            field: parse_text_argument(&args[2]),
            mode,
            index: None,
        };

        self.skip_ws();
        if matches!(self.peek_char(), Some('[')) {
            self.take_char();
            let index_expr = self.parse_expression()?;
            self.skip_ws();
            if !matches!(self.take_char(), Some(']')) {
                return Err("expected ']'".to_string());
            }
            if let IrExpr::ReqSeries { index, .. } = &mut expr {
                *index = Some(Box::new(index_expr));
            }
        }

        Ok(expr)
    }

    fn parse_identifier(&mut self) -> String {
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if is_ident_part(ch) {
                self.take_char();
            } else {
                break;
            }
        }
        self.source[start..self.pos].to_string()
    }
}

fn split_top_level_args(raw_args: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut depth_paren = 0usize;
    let mut depth_bracket = 0usize;
    let mut depth_brace = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut arg_start = 0usize;

    let bytes = raw_args.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let ch = bytes[i];
        if ch == b'\\' && in_string {
            escaped = !escaped;
            i += 1;
            continue;
        }
        if ch == b'"' && !escaped {
            in_string = !in_string;
            i += 1;
            continue;
        }
        escaped = false;
        if in_string {
            i += 1;
            continue;
        }

        match ch {
            b'(' => depth_paren = depth_paren.saturating_add(1),
            b')' => depth_paren = depth_paren.saturating_sub(1),
            b'[' => depth_bracket = depth_bracket.saturating_add(1),
            b']' => depth_bracket = depth_bracket.saturating_sub(1),
            b'{' => depth_brace = depth_brace.saturating_add(1),
            b'}' => depth_brace = depth_brace.saturating_sub(1),
            b',' if depth_paren == 0 && depth_bracket == 0 && depth_brace == 0 => {
                let arg = raw_args[arg_start..i].trim();
                if !arg.is_empty() {
                    args.push(arg.to_string());
                }
                arg_start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    if arg_start <= raw_args.len() {
        let tail = raw_args[arg_start..].trim();
        if !tail.is_empty() {
            args.push(tail.to_string());
        }
    }

    args
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_part(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'
}

fn map_series_field(ident: &str) -> Option<IrSeriesField> {
    match ident {
        "open" | "ctx.open" => Some(IrSeriesField::Open),
        "high" | "ctx.high" => Some(IrSeriesField::High),
        "low" | "ctx.low" => Some(IrSeriesField::Low),
        "close" | "ctx.close" => Some(IrSeriesField::Close),
        "volume" | "ctx.volume" => Some(IrSeriesField::Volume),
        "time" | "ctx.time" => Some(IrSeriesField::Time),
        "bar_index" | "ctx.bar_index" => Some(IrSeriesField::BarIndex),
        _ => None,
    }
}
