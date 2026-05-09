//! STEP ISO 10303-21 physical file parser.
//!
//! Parses the DATA section into a HashMap of entity instances.
//! Does not interpret entity semantics — that's the reader's job.

use std::collections::HashMap;

/// A parsed parameter value from a STEP entity.
#[derive(Clone, Debug)]
pub enum Param {
    /// Integer value (e.g., degree in B-spline)
    Int(i64),
    /// Real value (e.g., coordinates)
    Real(f64),
    /// String value (e.g., entity name)
    Str(String),
    /// Enumeration value (e.g., .T., .F., .UNSPECIFIED.)
    Enum(String),
    /// Reference to another entity (#123)
    Ref(u64),
    /// List of parameters
    List(Vec<Param>),
    /// Not provided ($)
    Omitted,
    /// Derived (*)
    Derived,
}

impl Param {
    pub fn as_ref(&self) -> Option<u64> {
        if let Param::Ref(id) = self { Some(*id) } else { None }
    }

    pub fn as_real(&self) -> Option<f64> {
        match self {
            Param::Real(v) => Some(*v),
            Param::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        if let Param::Int(v) = self { Some(*v) } else { None }
    }

    pub fn as_enum(&self) -> Option<&str> {
        if let Param::Enum(s) = self { Some(s) } else { None }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Param::Enum(s) => match s.as_str() {
                "T" => Some(true),
                "F" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Param]> {
        if let Param::List(v) = self { Some(v) } else { None }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let Param::Str(s) = self { Some(s) } else { None }
    }

    /// Extract a list of entity references.
    pub fn as_ref_list(&self) -> Option<Vec<u64>> {
        self.as_list().map(|list| {
            list.iter().filter_map(|p| p.as_ref()).collect()
        })
    }

    /// Extract a list of reals (e.g., coordinates).
    pub fn as_real_list(&self) -> Option<Vec<f64>> {
        self.as_list().map(|list| {
            list.iter().filter_map(|p| p.as_real()).collect()
        })
    }
}

/// A parsed STEP entity instance.
#[derive(Clone, Debug)]
pub struct Entity {
    pub id: u64,
    pub name: String,
    pub params: Vec<Param>,
}

/// Parsed STEP file: just the entity map from the DATA section.
pub struct StepFile {
    pub entities: HashMap<u64, Entity>,
}

impl StepFile {
    pub fn get(&self, id: u64) -> Option<&Entity> {
        self.entities.get(&id)
    }

    /// Find all entities with a given type name.
    pub fn entities_of_type(&self, name: &str) -> Vec<&Entity> {
        self.entities.values()
            .filter(|e| e.name.eq_ignore_ascii_case(name))
            .collect()
    }
}

/// Parse a STEP file from text.
pub fn parse_step(input: &str) -> Result<StepFile, String> {
    let mut entities = HashMap::new();

    // Find the DATA section
    let data_start = input.find("DATA;")
        .or_else(|| input.find("DATA ;"))
        .ok_or("no DATA section found")?;
    let data_end = input[data_start..].find("ENDSEC;")
        .ok_or("no ENDSEC after DATA")?;
    let data_section = &input[data_start + 5..data_start + data_end];

    // Strip comments
    let data_section = strip_comments(data_section);

    // Parse entity instances: #ID = NAME(params...);
    let mut pos = 0;
    let bytes = data_section.as_bytes();

    while pos < bytes.len() {
        // Skip whitespace
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() { break; }

        // Look for #
        if bytes[pos] != b'#' {
            pos += 1;
            continue;
        }
        pos += 1;

        // Parse entity ID
        let id_start = pos;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }
        if pos == id_start { continue; }
        let id: u64 = data_section[id_start..pos].parse().map_err(|e| format!("bad entity id: {}", e))?;

        // Skip whitespace and =
        while pos < bytes.len() && (bytes[pos].is_ascii_whitespace() || bytes[pos] == b'=') {
            pos += 1;
        }

        // Check for complex entity: #ID = ( TYPE1(...) TYPE2(...) ... );
        // or simple entity: #ID = NAME(params...);
        skip_ws(bytes, &mut pos);
        if pos >= bytes.len() { continue; }

        if bytes[pos] == b'(' {
            // Complex entity — multiple sub-entities sharing the same ID.
            // Parse each sub-entity inside the outer parens.
            pos += 1; // skip outer (
            while pos < bytes.len() && bytes[pos] != b')' {
                skip_ws(bytes, &mut pos);
                if pos >= bytes.len() || bytes[pos] == b')' { break; }

                // Parse sub-entity name
                let sub_name_start = pos;
                while pos < bytes.len() && bytes[pos] != b'(' && bytes[pos] != b')' && !bytes[pos].is_ascii_whitespace() {
                    pos += 1;
                }
                let sub_name = data_section[sub_name_start..pos].trim().to_uppercase();
                if sub_name.is_empty() { pos += 1; continue; }

                skip_ws(bytes, &mut pos);
                if pos < bytes.len() && bytes[pos] == b'(' {
                    // Has params — parse them
                    let sub_start = pos;
                    let sub_end = find_matching_paren(bytes, &mut pos);
                    let param_str = &data_section[sub_start..sub_end];
                    if let Ok(params) = parse_params(param_str) {
                        // Register under the same ID — last one with params wins
                        // for lookup purposes, but we register all names
                        if !params.is_empty() || !entities.contains_key(&id) {
                            entities.insert(id, Entity { id, name: sub_name, params });
                        }
                    }
                }
                skip_ws(bytes, &mut pos);
            }
            if pos < bytes.len() { pos += 1; } // skip outer )
        } else {
            // Simple entity
            let name_start = pos;
            while pos < bytes.len() && bytes[pos] != b'(' && !bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            let name = data_section[name_start..pos].to_uppercase();

            skip_ws(bytes, &mut pos);
            if pos >= bytes.len() || bytes[pos] != b'(' { continue; }

            let params_start = pos;
            let end = find_matching_paren(bytes, &mut pos);
            let param_str = &data_section[params_start..end];
            let params = parse_params(param_str)?;

            entities.insert(id, Entity { id, name, params });
        }

        // Skip to semicolon
        while pos < bytes.len() && bytes[pos] != b';' {
            pos += 1;
        }
        pos += 1;
    }

    Ok(StepFile { entities })
}

/// Find the matching closing paren, advancing `pos` past it.
fn find_matching_paren(bytes: &[u8], pos: &mut usize) -> usize {
    let mut depth = 0;
    let mut in_string = false;
    loop {
        if *pos >= bytes.len() { return *pos; }
        match bytes[*pos] {
            b'\'' if !in_string => in_string = true,
            b'\'' if in_string => {
                if *pos + 1 < bytes.len() && bytes[*pos + 1] == b'\'' {
                    *pos += 1;
                } else {
                    in_string = false;
                }
            }
            b'(' if !in_string => depth += 1,
            b')' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    *pos += 1;
                    return *pos;
                }
            }
            _ => {}
        }
        *pos += 1;
    }
}

/// Parse the parameter list inside parentheses.
fn parse_params(input: &str) -> Result<Vec<Param>, String> {
    let trimmed = input.trim();
    if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
        return Ok(Vec::new());
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    parse_param_list(inner)
}

fn parse_param_list(input: &str) -> Result<Vec<Param>, String> {
    let mut result = Vec::new();
    let mut pos = 0;
    let bytes = input.as_bytes();

    while pos < bytes.len() {
        skip_ws(bytes, &mut pos);
        if pos >= bytes.len() { break; }

        let param = parse_one_param(input, bytes, &mut pos)?;
        result.push(param);

        skip_ws(bytes, &mut pos);
        if pos < bytes.len() && bytes[pos] == b',' {
            pos += 1;
        }
    }

    Ok(result)
}

fn parse_one_param(input: &str, bytes: &[u8], pos: &mut usize) -> Result<Param, String> {
    skip_ws(bytes, pos);
    if *pos >= bytes.len() { return Ok(Param::Omitted); }

    match bytes[*pos] {
        b'$' => { *pos += 1; Ok(Param::Omitted) }
        b'*' => { *pos += 1; Ok(Param::Derived) }
        b'#' => {
            *pos += 1;
            let start = *pos;
            while *pos < bytes.len() && bytes[*pos].is_ascii_digit() { *pos += 1; }
            let id: u64 = input[start..*pos].parse().map_err(|e| format!("bad ref: {}", e))?;
            Ok(Param::Ref(id))
        }
        b'.' => {
            // Enumeration: .NAME.
            *pos += 1;
            let start = *pos;
            while *pos < bytes.len() && bytes[*pos] != b'.' { *pos += 1; }
            let name = input[start..*pos].to_string();
            if *pos < bytes.len() { *pos += 1; } // skip closing .
            Ok(Param::Enum(name))
        }
        b'\'' => {
            // String
            *pos += 1;
            let mut s = String::new();
            while *pos < bytes.len() {
                if bytes[*pos] == b'\'' {
                    if *pos + 1 < bytes.len() && bytes[*pos + 1] == b'\'' {
                        s.push('\'');
                        *pos += 2;
                    } else {
                        *pos += 1;
                        break;
                    }
                } else {
                    s.push(bytes[*pos] as char);
                    *pos += 1;
                }
            }
            Ok(Param::Str(s))
        }
        b'(' => {
            // Nested list
            *pos += 1;
            let mut items = Vec::new();
            loop {
                skip_ws(bytes, pos);
                if *pos >= bytes.len() || bytes[*pos] == b')' {
                    if *pos < bytes.len() { *pos += 1; }
                    break;
                }
                items.push(parse_one_param(input, bytes, pos)?);
                skip_ws(bytes, pos);
                if *pos < bytes.len() && bytes[*pos] == b',' { *pos += 1; }
            }
            Ok(Param::List(items))
        }
        c if c == b'-' || c == b'+' || c.is_ascii_digit() => {
            // Number (int or real)
            let start = *pos;
            if bytes[*pos] == b'-' || bytes[*pos] == b'+' { *pos += 1; }
            while *pos < bytes.len() && bytes[*pos].is_ascii_digit() { *pos += 1; }
            let mut is_real = false;
            if *pos < bytes.len() && bytes[*pos] == b'.' {
                is_real = true;
                *pos += 1;
                while *pos < bytes.len() && bytes[*pos].is_ascii_digit() { *pos += 1; }
            }
            if *pos < bytes.len() && (bytes[*pos] == b'e' || bytes[*pos] == b'E') {
                is_real = true;
                *pos += 1;
                if *pos < bytes.len() && (bytes[*pos] == b'-' || bytes[*pos] == b'+') { *pos += 1; }
                while *pos < bytes.len() && bytes[*pos].is_ascii_digit() { *pos += 1; }
            }
            let s = &input[start..*pos];
            if is_real {
                Ok(Param::Real(s.parse().map_err(|e| format!("bad real '{}': {}", s, e))?))
            } else {
                Ok(Param::Int(s.parse().map_err(|e| format!("bad int '{}': {}", s, e))?))
            }
        }
        _ => {
            // Skip unknown token
            *pos += 1;
            Ok(Param::Omitted)
        }
    }
}

fn skip_ws(bytes: &[u8], pos: &mut usize) {
    while *pos < bytes.len() && bytes[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
}

fn strip_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_step() {
        let input = r#"
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('test'),'2;1');
ENDSEC;
DATA;
#1=CARTESIAN_POINT('',(1.0,2.0,3.0));
#2=DIRECTION('',(0.,0.,1.));
#3=AXIS2_PLACEMENT_3D('',#1,#2,$);
#4=PLANE('',#3);
ENDSEC;
END-ISO-10303-21;
"#;
        let step = parse_step(input).unwrap();
        assert_eq!(step.entities.len(), 4);

        let pt = step.get(1).unwrap();
        assert_eq!(pt.name, "CARTESIAN_POINT");
        let coords = pt.params[1].as_real_list().unwrap();
        assert_eq!(coords, vec![1.0, 2.0, 3.0]);

        let dir = step.get(2).unwrap();
        assert_eq!(dir.name, "DIRECTION");

        let axis = step.get(3).unwrap();
        assert_eq!(axis.name, "AXIS2_PLACEMENT_3D");
        assert_eq!(axis.params[1].as_ref(), Some(1));
        assert_eq!(axis.params[2].as_ref(), Some(2));
        assert!(matches!(axis.params[3], Param::Omitted));

        let plane = step.get(4).unwrap();
        assert_eq!(plane.name, "PLANE");
    }

    #[test]
    fn parse_booleans_and_enums() {
        let input = r#"
ISO-10303-21;
HEADER;
ENDSEC;
DATA;
#1=ADVANCED_FACE('',(#10,#20),#30,.T.);
#2=ORIENTED_EDGE('',*,*,#40,.F.);
ENDSEC;
END-ISO-10303-21;
"#;
        let step = parse_step(input).unwrap();

        let face = step.get(1).unwrap();
        assert_eq!(face.params[3].as_bool(), Some(true));

        let oe = step.get(2).unwrap();
        assert!(matches!(oe.params[1], Param::Derived));
        assert_eq!(oe.params[4].as_bool(), Some(false));
    }

    #[test]
    fn parse_nested_lists() {
        let input = r#"
ISO-10303-21;
HEADER;
ENDSEC;
DATA;
#1=B_SPLINE_SURFACE_WITH_KNOTS('',1,1,((#10,#11),(#12,#13)),.UNSPECIFIED.,.F.,.F.,.F.,(2,2),(2,2),(0.,1.),(0.,1.),.UNSPECIFIED.);
ENDSEC;
END-ISO-10303-21;
"#;
        let step = parse_step(input).unwrap();
        let bss = step.get(1).unwrap();
        assert_eq!(bss.name, "B_SPLINE_SURFACE_WITH_KNOTS");
        // degree u = 1
        assert_eq!(bss.params[1].as_int(), Some(1));
    }
}
