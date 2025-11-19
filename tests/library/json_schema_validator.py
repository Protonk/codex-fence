#!/usr/bin/env python3
"""
Minimal JSON Schema validator used by tests to confirm boundary objects adhere
to schema/boundary_object.json without introducing third-party dependencies.
Supports the subset of the spec exercised by cfbo-v1: type checks, const/enum,
required properties, additionalProperties, arrays/items, uniqueItems, and $ref.
"""
import json
import sys
from typing import Any, Dict, List


def load_json(path: str) -> Any:
  with open(path, "r", encoding="utf-8") as handle:
    return json.load(handle)


def type_matches(instance: Any, type_spec: Any) -> bool:
  allowed: List[str]
  if isinstance(type_spec, list):
    allowed = type_spec
  else:
    allowed = [type_spec]
  for candidate in allowed:
    if candidate == "string" and isinstance(instance, str):
      return True
    if candidate == "integer" and isinstance(instance, int) and not isinstance(instance, bool):
      return True
    if candidate == "number" and isinstance(instance, (int, float)) and not isinstance(instance, bool):
      return True
    if candidate == "boolean" and isinstance(instance, bool):
      return True
    if candidate == "object" and isinstance(instance, dict):
      return True
    if candidate == "array" and isinstance(instance, list):
      return True
    if candidate == "null" and instance is None:
      return True
  return False


def describe(instance: Any) -> str:
  if instance is None:
    return "null"
  if isinstance(instance, bool):
    return "boolean"
  if isinstance(instance, int):
    return "integer"
  if isinstance(instance, float):
    return "number"
  if isinstance(instance, str):
    return "string"
  if isinstance(instance, list):
    return "array"
  if isinstance(instance, dict):
    return "object"
  return type(instance).__name__


def resolve_ref(schema: Dict[str, Any], ref: str) -> Dict[str, Any]:
  if not ref.startswith("#/"):
    raise ValueError(f"Unsupported $ref target: {ref}")
  target: Any = schema
  for part in ref.lstrip("#/").split("/"):
    if part not in target:
      raise KeyError(f"$ref path '{ref}' segment '{part}' missing from schema")
    target = target[part]
  if not isinstance(target, dict):
    raise TypeError(f"$ref '{ref}' does not resolve to an object")
  return target


def validate(instance: Any, schema: Dict[str, Any], root_schema: Dict[str, Any], path: str, errors: List[str]) -> None:
  if "$ref" in schema:
    ref_schema = resolve_ref(root_schema, schema["$ref"])
    validate(instance, ref_schema, root_schema, path, errors)
    return

  type_spec = schema.get("type")
  if type_spec is not None:
    if not type_matches(instance, type_spec):
      errors.append(f"{path}: expected type {type_spec}, got {describe(instance)}")
      return

  if "const" in schema and instance != schema["const"]:
    errors.append(f"{path}: expected const {schema['const']}, got {instance!r}")

  if "enum" in schema and instance not in schema["enum"]:
    errors.append(f"{path}: expected one of {schema['enum']}, got {instance!r}")

  if isinstance(type_spec, list):
    allowed_types = set(type_spec)
  elif isinstance(type_spec, str):
    allowed_types = {type_spec}
  else:
    allowed_types = set()

  object_keywords = {"properties", "required", "additionalProperties"}
  if object_keywords.intersection(schema.keys()) or "object" in allowed_types:
    if not isinstance(instance, dict):
      errors.append(f"{path}: expected object, got {describe(instance)}")
      return
    props = schema.get("properties", {})
    for key in schema.get("required", []):
      if key not in instance:
        errors.append(f"{path}.{key}: missing required property")
    additional = schema.get("additionalProperties", True)
    for key, value in instance.items():
      if key in props:
        validate(value, props[key], root_schema, f"{path}.{key}", errors)
      else:
        if additional is False:
          errors.append(f"{path}.{key}: additional properties not allowed")
        elif isinstance(additional, dict):
          validate(value, additional, root_schema, f"{path}.{key}", errors)

  if "array" in allowed_types or "items" in schema or "uniqueItems" in schema:
    if not isinstance(instance, list):
      errors.append(f"{path}: expected array, got {describe(instance)}")
      return
    item_schema = schema.get("items")
    if isinstance(item_schema, dict):
      for idx, item in enumerate(instance):
        validate(item, item_schema, root_schema, f"{path}[{idx}]", errors)
    if schema.get("uniqueItems"):
      seen = set()
      for idx, item in enumerate(instance):
        marker = json.dumps(item, sort_keys=True)
        if marker in seen:
          errors.append(f"{path}[{idx}]: duplicate array entry violates uniqueItems")
        seen.add(marker)


def main() -> int:
  if len(sys.argv) != 3:
    print("Usage: json_schema_validator.py <schema_path> <json_path>", file=sys.stderr)
    return 1
  schema_path, json_path = sys.argv[1:]
  schema = load_json(schema_path)
  instance = load_json(json_path)
  errors: List[str] = []
  validate(instance, schema, schema, "$", errors)
  if errors:
    for err in errors:
      print(f"schema validation error: {err}", file=sys.stderr)
    return 1
  return 0


if __name__ == "__main__":
  sys.exit(main())
