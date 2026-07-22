//! Scripting Ecosystem
//!
//! Provides an embedded scripting environment for extending the
//! simulation kernel with user-defined logic, custom blocks,
//! and automation scripts.

use std::collections::HashMap;

/// A generic script value type for the scripting environment.
#[derive(Debug, Clone)]
pub enum ScriptValue {
    Number(f64),
    Integer(i64),
    Boolean(bool),
    String(String),
    Array(Vec<ScriptValue>),
    Map(HashMap<String, ScriptValue>),
    None,
}

/// A variable in the scripting environment.
#[derive(Debug, Clone)]
pub struct ScriptVariable {
    pub name: String,
    pub value: ScriptValue,
    pub mutable: bool,
}

/// The scripting environment — holds variables and provides
/// execution context for user scripts.
#[derive(Debug, Default)]
pub struct ScriptEnvironment {
    variables: HashMap<String, ScriptVariable>,
}

impl ScriptEnvironment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, name: &str, value: ScriptValue) {
        if let Some(var) = self.variables.get(name) {
            if var.mutable {
                self.variables.insert(name.to_string(), ScriptVariable {
                    name: name.to_string(),
                    value,
                    mutable: true,
                });
            }
        } else {
            self.variables.insert(name.to_string(), ScriptVariable {
                name: name.to_string(),
                value,
                mutable: true,
            });
        }
    }

    pub fn get(&self, name: &str) -> Option<&ScriptValue> {
        self.variables.get(name).map(|v| &v.value)
    }

    pub fn declare(&mut self, name: &str, value: ScriptValue, mutable: bool) {
        self.variables.insert(name.to_string(), ScriptVariable {
            name: name.to_string(),
            value,
            mutable,
        });
    }

    pub fn has(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    pub fn clear(&mut self) {
        self.variables.clear();
    }
}
