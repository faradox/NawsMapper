//! Field-level change propagation, the engine behind multi-selection editing.
//!
//! The properties panel edits a copy of the *primary* selected item. We then
//! diff "before" against "after" and apply only the fields that actually
//! changed to every other selected item, so unrelated per-item values (e.g.
//! each surface's own phase offset) are preserved.
//!
//! This works through `serde_json::Value`, so new fields participate
//! automatically without any per-field code.

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

/// Applies every field that differs between `before` and `after` to `target`.
///
/// Objects are diffed recursively; anything else (numbers, arrays such as
/// colors or vectors, enum variants) is replaced as a whole.
pub fn propagate_changes<T>(before: &T, after: &T, target: &mut T) -> anyhow::Result<()>
where
    T: Serialize + DeserializeOwned,
{
    let before = serde_json::to_value(before)?;
    let after = serde_json::to_value(after)?;
    let mut tgt = serde_json::to_value(&*target)?;
    if apply(&before, &after, &mut tgt) {
        *target = serde_json::from_value(tgt)?;
    }
    Ok(())
}

fn apply(before: &Value, after: &Value, target: &mut Value) -> bool {
    if before == after {
        return false;
    }
    match (before, after, &mut *target) {
        (Value::Object(b), Value::Object(a), Value::Object(t)) => {
            let mut changed = false;
            for (key, a_val) in a {
                match (b.get(key), t.get_mut(key)) {
                    (Some(b_val), Some(t_val)) => changed |= apply(b_val, a_val, t_val),
                    _ => {
                        t.insert(key.clone(), a_val.clone());
                        changed = true;
                    }
                }
            }
            changed
        }
        _ => {
            *target = after.clone();
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    struct Inner {
        a: f32,
        b: f32,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    struct Outer {
        name: String,
        inner: Inner,
        color: [f32; 4],
    }

    #[test]
    fn only_changed_fields_propagate() {
        let before = Outer { name: "a".into(), inner: Inner { a: 1.0, b: 2.0 }, color: [0.0; 4] };
        let mut after = before.clone();
        after.inner.b = 5.0;

        let mut target = Outer { name: "t".into(), inner: Inner { a: 9.0, b: 9.0 }, color: [1.0; 4] };
        propagate_changes(&before, &after, &mut target).unwrap();
        assert_eq!(target.name, "t");
        assert_eq!(target.inner, Inner { a: 9.0, b: 5.0 });
        assert_eq!(target.color, [1.0; 4]);
    }

    #[test]
    fn arrays_are_replaced_whole() {
        let before = Outer { name: "a".into(), inner: Inner { a: 1.0, b: 2.0 }, color: [0.0; 4] };
        let mut after = before.clone();
        after.color[0] = 1.0;
        let mut target = before.clone();
        target.color = [0.5; 4];
        propagate_changes(&before, &after, &mut target).unwrap();
        assert_eq!(target.color, [1.0, 0.0, 0.0, 0.0]);
    }
}
