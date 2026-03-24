# Pegasus Serialization Edge Cases

Reverse-engineered from decompiled `com.linkedin.data.lite` and `com.linkedin.android.pegasus.gen`
model classes. All findings are from the jadx decompilation of the LinkedIn Android APK.

---

## 1. Null vs Absent Field Distinction

Pegasus uses a **two-flag system** to distinguish absent fields from present-but-null fields.
Every record model has a parallel `boolean hasXxx` flag for each field `xxx`.

### Serialization (accept → DataProcessor)

When a model serializes itself via `accept(DataProcessor)`, fields are **omitted entirely** if
their `hasXxx` flag is `false`. There is no explicit null emitted for absent fields.

For reference-type fields (records, strings, unions, collections), the guard is:

```java
if (this.hasShareAudience && this.shareAudience != null) {
    dataProcessor.startRecordField("shareAudience", 6091);
    dataProcessor.processEnum(this.shareAudience);
    dataProcessor.endRecordField();
}
```

For primitive fields (long, boolean, int), only `hasXxx` is checked (no null check needed):

```java
if (this.hasCreatedTime) {
    dataProcessor.startRecordField("createdTime", 5291);
    dataProcessor.processLong(this.createdTime);
    dataProcessor.endRecordField();
}
```

Source: `Reshare.accept()`, `Event.accept()`, `ChangeTimeStamps.accept()`

### Deserialization (Builder → DataReader)

When reading from JSON/protobuf, the builder checks `isNullNext()` before reading each field.
If null is encountered, it calls `skipValue()` and sets `hasXxx = false`:

```java
case 1498: // createdAt
    if (dataReader.isNullNext()) {
        dataReader.skipValue();
        z3 = false;            // hasCreatedAt = false
    } else {
        j = dataReader.readLong();
        z3 = true;             // hasCreatedAt = true
    }
```

Source: `EventBuilder.build(DataReader)`

### The shouldHandleExplicitNulls Flag

The `DataProcessor` interface has a `shouldHandleExplicitNulls()` method. This controls
whether the serializer will call `processNull()` when a field value is null but `has*` is true.

| Serializer | shouldHandleExplicitNulls | Behavior |
|---|---|---|
| `JSONObjectGenerator` | Configurable (constructor arg `_serializeNull`) | When true, emits `JSONObject.NULL`; when false, omits field |
| `JacksonJsonGenerator` | `true` (hardcoded) | Always writes `null` for null-valued present fields |
| `ProtobufGenerator` | `true` (hardcoded) | Writes byte `0x0B` for null |
| `RawDataGenerator` | `true` (hardcoded) | Always includes nulls |
| `AbstractDataProcessor` (base) | `false` (default) | Throws on `processNull()` |

In `RawDataProcessorUtil.processObject()`:

```java
if (obj == null && dataProcessor.shouldHandleExplicitNulls()) {
    dataProcessor.processNull();
    return null;
}
```

Same pattern applies to `processList()` and `processMap()`.

### Practical Implication for Rust Client

**When deserializing JSON responses**: A field can be:
1. **Absent from JSON** -- treat as not present (`has = false`)
2. **Present with value `null`** -- treat as not present (`has = false`)
3. **Present with a value** -- treat as present (`has = true`)

Cases 1 and 2 are **equivalent** from the client's perspective. The app treats both
as `hasXxx = false`. Use `Option<T>` in Rust and deserialize both absent and null as `None`.

**When serializing request bodies**: Omit fields entirely when they have no value. Do NOT
send `"field": null`. The Jackson serializer does emit nulls for present-but-null-valued
fields, but in practice the model's `accept()` method skips fields where `has* = false`,
so null is only emitted if a field is explicitly set to null (which is unusual in practice).

---

## 2. Empty Collection Handling

Collections have special treatment with three distinct states:

### The Three States

1. **Absent** (`hasActions = false`): No field emitted during serialization
2. **Explicitly empty** (`hasActionsExplicitDefaultSet = true`): Builder tracks this via
   a separate boolean; the field IS emitted as an empty array `[]`
3. **Non-empty** (`hasActions = true`, list has elements): Normal array serialization

### Builder Logic

```java
public Builder setActions(List<UpdateAction> list) {
    boolean z = list != null && list.equals(Collections.emptyList());
    this.hasActionsExplicitDefaultSet = z;            // tracks explicit empty
    boolean z2 = (list == null || z) ? false : true;
    this.hasActions = z2;                              // only true if non-empty
    if (!z2) {
        list = Collections.emptyList();                // normalize to emptyList
    }
    this.actions = list;
    return this;
}
```

In RECORD flavor, if `hasActions` is false, the list is set to `Collections.emptyList()`:

```java
if (!this.hasActions) {
    this.actions = Collections.emptyList();
}
```

In PARTIAL flavor, `hasActions` includes `hasActionsExplicitDefaultSet`:

```java
this.hasActions || this.hasActionsExplicitDefaultSet
```

Source: `Reshare.Builder`

### Serialization Guard

```java
if (!this.hasActions || this.actions == null) {
    list = null;      // field omitted entirely
} else {
    dataProcessor.startRecordField("actions", 5206);
    list = RawDataProcessorUtil.processList(this.actions, dataProcessor, null, 1, 0);
    dataProcessor.endRecordField();
}
```

### Practical Implication for Rust Client

- **Deserializing**: An absent array field and a `[]` value should both result in an empty
  `Vec<T>`. The app normalizes to `Collections.emptyList()` in both cases.
- **Serializing**: Omit empty collections from request bodies. If you need to explicitly
  clear a collection (e.g., PATCH operations), send `[]` -- but this is rare.
- The `unmodifiableList()` wrapper is applied at construction time, so all list fields
  in the model are immutable after building.

---

## 3. Timestamp Format

**All timestamps are epoch milliseconds** stored as `long` values.

### Evidence

1. **Field types**: All timestamp fields (`createdAt`, `expiresAt`, `createdTime`,
   `lastModified`, `deleted`) are typed as `long` in the model classes, read via
   `dataReader.readLong()`, and written via `dataProcessor.processLong()`.

2. **No conversion code**: There is no division by 1000, no `TimeUnit` conversion, and no
   `Date` constructor wrapping in the model layer. The raw long value is stored and
   transmitted directly.

3. **Standard LinkedIn/Java convention**: Java's `System.currentTimeMillis()` returns
   epoch milliseconds, and LinkedIn's Pegasus framework follows this convention. The
   `ChangeTimeStamps` model (which is a standard Rest.li schema) uses raw longs for
   `created`, `lastModified`, and `deleted`.

4. **Field names**: Fields are named `createdAt`, `expiresAt`, `lastModified` -- consistent
   with millisecond-precision timestamps. There are no fields named with `Seconds` or `Secs`
   suffixes.

### Wire Format

- **JSON**: Timestamps are serialized as JSON numbers (unquoted integers):
  `"createdAt": 1632945600000`
- **Protobuf**: Timestamps are serialized as varint-encoded int64 (type tag `0x05` for
  long values).

### Practical Implication for Rust Client

Use `i64` for all timestamp fields. The value is Unix epoch milliseconds. To convert to
a `chrono::DateTime`, divide by 1000 for seconds and take modulo 1000 for sub-second
milliseconds, or use `chrono::DateTime::from_timestamp_millis()`.

---

## 4. Nested Union Discriminator Format

### Union Wire Format (JSON)

A union is serialized as a JSON object with a **single key** that is the fully-qualified
type name (FQN) of the active member, and the value is the member's serialized form:

```json
{
  "com.linkedin.voyager.feed.ShareArticle": {
    "title": "...",
    "url": "..."
  }
}
```

### Discriminator Key Format

The discriminator key is the **Pegasus schema FQN**, not the Java class name. Specifically:

- Uses dots as separators: `com.linkedin.voyager.messaging.event.message.ForwardedContent`
- Does NOT include the `android.pegasus.gen` package prefix that the Java classes use
- The FQN is the **same string** used in `startUnionMember()` during serialization:

```java
dataProcessor.startUnionMember(
    "com.linkedin.voyager.messaging.event.message.ForwardedContent", 897);
```

Source: `CustomContent.accept()`

### Ordinal System

Each union member also has a numeric ordinal (the second argument to `startUnionMember()`).
This ordinal is used by:

1. **Protobuf format**: The ordinal can replace the string FQN via the symbol table for
   compact encoding
2. **HashStringKeyStore**: Maps FQN strings to ordinals for O(1) lookup during deserialization

The ordinals are assigned per-union (not globally) and are stable within an app version.

### Nesting Behavior

When a record field is itself a union, and that union member is a record containing another
union, the nesting follows naturally:

```json
{
  "eventContent": {
    "com.linkedin.voyager.messaging.event.message.MessageEvent": {
      "customContent": {
        "com.linkedin.voyager.feed.ShareArticle": {
          "title": "..."
        }
      }
    }
  }
}
```

Each level of union introduces one layer of `{ "fqn": value }` wrapping. There is no
flattening or special handling for deeply nested unions. The `startUnion()`/`endUnion()` calls
in the DataProcessor translate to `startMap()`/`endMap()` in both JSON generators, meaning
each union is just a JSON object.

### Deserialization of Union Members

The builder uses `startRecord()` + `nextFieldOrdinal(JSON_KEY_STORE)` to read the union's
single key-value pair. It counts members and validates that exactly one is present:

```java
if ((dataReader instanceof FissionDataReader) && i != 1) {
    throw new DataReaderException("Invalid union. Found " + i + " members");
}
```

Note: This strict validation only applies to `FissionDataReader` (the local cache reader).
The JSON parser path does not enforce single-member validation, meaning the server could
technically send multiple members (though this would violate the Pegasus spec).

### Union with Zero Members (Empty Union)

If no member is present, all `hasXxxValue` flags are false and all values are null.
The builder still constructs the union object. The `validateUnionMemberCount()` method
in `AbstractUnionTemplateBuilder` only checks for **more than one** member, not zero:

```java
public void validateUnionMemberCount(boolean... zArr) throws UnionMemberCountException {
    int i = 0;
    for (boolean z : zArr) { if (z) i++; }
    if (i > 1) {
        throw new UnionMemberCountException(getClass().getName(), i);
    }
}
```

So a union with zero members is valid (represents "none of the above" / unset).

### Practical Implication for Rust Client

Model unions as Rust enums with a `None` / `Unknown` variant:

```rust
enum CustomContent {
    ShareArticle(ShareArticle),
    ForwardedContent(ForwardedContent),
    // ... other members
    None,  // zero members present
}
```

When deserializing JSON, parse the single key of the union object to determine the variant.
Use the FQN string (e.g., `"com.linkedin.voyager.feed.ShareArticle"`) as the match key.
Unknown FQNs should map to an `Unknown(String, serde_json::Value)` variant for
forward-compatibility.

---

## 5. Custom Serializers Beyond Standard Template

### Serializer Implementations

There are five concrete `DataProcessor`/`DataTemplateSerializer` implementations:

| Class | Purpose | Format |
|---|---|---|
| `JacksonJsonGenerator` | Streaming JSON output via Jackson | JSON |
| `JSONObjectGenerator` | In-memory `JSONObject` construction | JSON (in-memory) |
| `RawDataGenerator` | In-memory `Map`/`List` tree | Raw Java objects |
| `ProtobufGenerator` | LinkedIn custom protobuf format | Binary (x-protobuf2) |
| `FissionProtobufSerializer.FissionProtobufGenerator` | Cache-aware protobuf with lookup tables | Binary (cache) |

### Coercer System

The `Coercer<CUSTOM, RAW>` interface handles type conversions that are not part of the
standard primitive/record/union model:

| Coercer | Custom Type | Raw Type | Usage |
|---|---|---|---|
| `UrnCoercer` | `Urn` | `String` | URN fields are stored as `Urn` objects but serialized as plain strings |
| `BytesCoercer` | (not examined) | (not examined) | Binary data handling |

Coercers are invoked at specific depths in the type hierarchy via `RawDataProcessorUtil`:

```java
if (i == i2 && coercer != null) {
    Object processObject = processObject(
        coercer.coerceFromCustomType(obj), dataProcessor, coercer, i, i2 + 1);
}
```

### AnyRecord

The `AnyRecord` type is a special record that wraps an arbitrary type-erased value. It allows
fields to hold "any" Pegasus type without compile-time knowledge of the schema. This is used
for generic/dynamic data structures.

### Flavor System

Records can be built in three flavors, affecting validation:

| Flavor | Behavior |
|---|---|
| `RECORD` | Full validation: required fields enforced, defaults applied for missing collections |
| `PARTIAL` | Relaxed validation: required fields not enforced, explicit-default tracking honored |
| `PATCH` | (Defined but usage not traced in depth) |

The `PARTIAL` flavor is significant for decoration/projection responses where the server may
omit fields that the recipe did not request.

---

## 6. Protobuf Symbol Table Format

### Overview

LinkedIn uses a custom protobuf format (`application/x-protobuf2`) with a shared symbol
table for string compression. Field names, type FQNs, and enum values that appear in the
symbol table are replaced by their numeric index.

### Symbol Table Structure

The symbol table is a compile-time-generated bidirectional mapping:

```java
// From SymbolTableHolder.GeneratedSymbolTable
String[] strArr = new String[6673];         // index → name
HashMap hashMap = new HashMap(8898);         // name → index
// populated via populateSymbols0..32() methods
```

- **Size**: 6,673 symbols in the examined APK version (v6.1.1)
- **Hash code**: `1420265035` (used for table version matching)
- **Naming**: Advertised to the server as `zephyr-6673` (prefix `zephyr-` + size)

### Wire Protocol for Strings

In the protobuf format, each string value is preceded by a type tag byte:

| Tag | Meaning | Data |
|---|---|---|
| `0x02` | Inline UTF-8 string | length-prefixed string bytes |
| `0x03` | Symbol table reference | varint symbol ID (index into shared table) |
| `0x0F` | Local symbol table reference | varint index into per-message local symbol table |
| `0x14` | ASCII-only string | length-prefixed ASCII bytes (optimization) |

### Field Name Resolution

During deserialization, `ProtobufParser.nextFieldOrdinal()` resolves field names:

```java
byte tag = this._protoReader.readRawByte();
if (tag == 2) {
    return jsonKeyStore.getOrdinal(this._protoReader.readString());   // inline string
} else if (tag == 3) {
    int symbolId = this._protoReader.readInt32();
    // If symbol table hashes match, use symbolId directly as ordinal
    // Otherwise, look up the name and re-resolve
    return this._symbolTableHashCode == jsonKeyStore.hashCode()
        ? symbolId
        : jsonKeyStore.getOrdinal(this._symbolTable.getSymbolName(symbolId));
}
```

The key optimization: when the model's `JsonKeyStore` was built with the same symbol table
hash as the parser's symbol table, the symbol ID can be used directly as the field ordinal
without any string lookup.

### Local Symbol Tables

Individual protobuf messages can include a local symbol table (tag `0x0E`) that defines
message-scoped string mappings. These are loaded before the message fields:

```java
// In ProtobufParser.startMap():
if (peekRawByte == 14) {  // local symbol table
    int count = this._protoReader.readInt32();
    String[] localSymbols = new String[count];
    for (int i = 0; i < count; i++) {
        localSymbols[i] = this._protoReader.readString();
    }
    this._dataTemplateCache.loadLocalSymbols(localSymbols);
}
```

### Included Records (Deduplication)

Before field data, a message can include pre-parsed records (tag `0x0C`) for
deduplication. These are referenced later by index (tag `0x0D` / byte 13):

```java
if (peekRawByte == 12) {  // included records
    int count = this._protoReader.readInt32();
    byte[][] records = new byte[count][];
    for (int i = 0; i < count; i++) {
        records[i] = this._protoReader.readByteArray();
    }
    this._dataTemplateCache.loadIncludedRecords(records);
}
```

### Null Encoding

In protobuf, null is a single byte `0x0B`.

### Practical Implication for Rust Client

If using JSON (`Accept: application/json`), the symbol table is irrelevant -- the server
sends plain JSON with string field names. The symbol table only matters for the protobuf
wire format. If protobuf support is needed later:

1. Extract the full symbol table from the APK (6,673 entries from `SymbolTableHolder`)
2. Send `x-restli-symbol-table-name: zephyr-{N}` header with requests
3. Implement the tag-based string decoding (tags 0x02, 0x03, 0x0F, 0x14)
4. Handle local symbol tables and included records for deduplication

---

## 7. Wire Format Type Tags (Protobuf)

Complete tag reference for LinkedIn's custom protobuf format:

| Byte Tag | Type | Data Following |
|---|---|---|
| `0x00` | Map/Record start | varint field count |
| `0x01` | Array start | varint element count |
| `0x02` | UTF-8 string | length-prefixed bytes |
| `0x03` | Symbol table string | varint symbol ID |
| `0x04` | Int32 | varint (zigzag encoded) |
| `0x05` | Int64 | varint (zigzag encoded) |
| `0x06` | Float32 | 4 bytes (IEEE 754) |
| `0x07` | Float64 | 8 bytes (IEEE 754) |
| `0x08` | Boolean true | (no data) |
| `0x09` | Boolean false | (no data) |
| `0x0A` | Raw bytes | length-prefixed bytes |
| `0x0B` | Null | (no data) |
| `0x0C` | Included records | varint count + byte arrays |
| `0x0D` | Record reference | varint index / string ID |
| `0x0E` | Local symbol table | varint count + strings |
| `0x0F` | Local symbol reference | varint local index |
| `0x10` | (Message header/marker) | (context-dependent) |
| `0x14` | ASCII-only string | length-prefixed ASCII bytes |

Source: `ProtobufParser`, `ProtobufGenerator`

---

## Source Files

Key decompiled sources referenced:

- `com.linkedin.data.lite.DataProcessor` -- processor interface with null/union/record lifecycle
- `com.linkedin.data.lite.AbstractDataProcessor` -- base with type dispatch
- `com.linkedin.data.lite.JSONObjectGenerator` -- in-memory JSON serialization
- `com.linkedin.data.lite.jackson.JacksonJsonGenerator` -- streaming JSON serialization
- `com.linkedin.data.lite.protobuf.ProtobufParser` -- protobuf deserialization
- `com.linkedin.data.lite.protobuf.ProtobufGenerator` -- protobuf serialization
- `com.linkedin.data.lite.RawDataProcessorUtil` -- null handling, list/map/union dispatch
- `com.linkedin.data.lite.AbstractUnionTemplateBuilder` -- union member count validation
- `com.linkedin.data.lite.symbols.InMemorySymbolTable` -- symbol table implementation
- `com.linkedin.symbols.SymbolTableHolder` -- generated symbol table (6,673 entries)
- `com.linkedin.android.pegasus.gen.voyager.feed.Reshare` -- concrete record example
- `com.linkedin.android.pegasus.gen.voyager.messaging.event.message.CustomContent` -- concrete union example
- `com.linkedin.android.pegasus.gen.voyager.messaging.EventBuilder` -- deserialization example
- `com.linkedin.android.pegasus.gen.common.ChangeTimeStamps` -- timestamp model
- `com.linkedin.android.pegasus.gen.common.UrnCoercer` -- coercer example
