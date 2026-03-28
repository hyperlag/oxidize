/**
 * A simple recursive-descent JSON parser that parses JSON strings and
 * produces a tree of values. Exercises: string manipulation, recursion,
 * OOP (inheritance via toString), control flow, exception handling, and
 * ArrayList/HashMap usage.
 */
public class JsonParser {
    private String input;
    private int pos;

    public JsonParser(String input) {
        this.input = input;
        this.pos = 0;
    }

    private char peek() {
        skipWhitespace();
        if (pos >= input.length()) {
            return '\0';
        }
        return input.charAt(pos);
    }

    private char advance() {
        char c = input.charAt(pos);
        pos++;
        return c;
    }

    private void skipWhitespace() {
        while (pos < input.length()) {
            char c = input.charAt(pos);
            if (c == ' ' || c == '\t' || c == '\n' || c == '\r') {
                pos++;
            } else {
                break;
            }
        }
    }

    private void expect(char expected) {
        skipWhitespace();
        char c = advance();
        if (c != expected) {
            System.out.println("ERROR: expected '" + expected + "' but got '" + c + "'");
        }
    }

    public String parseString() {
        expect('"');
        StringBuilder sb = new StringBuilder();
        while (pos < input.length()) {
            char c = advance();
            if (c == '"') {
                return sb.toString();
            }
            if (c == '\\') {
                char next = advance();
                if (next == 'n') {
                    sb.append('\n');
                } else if (next == 't') {
                    sb.append('\t');
                } else if (next == '"') {
                    sb.append('"');
                } else if (next == '\\') {
                    sb.append('\\');
                }
            } else {
                sb.append(c);
            }
        }
        return sb.toString();
    }

    public int parseNumber() {
        skipWhitespace();
        int start = pos;
        boolean negative = false;
        if (pos < input.length() && input.charAt(pos) == '-') {
            negative = true;
            pos++;
        }
        while (pos < input.length() && input.charAt(pos) >= '0' && input.charAt(pos) <= '9') {
            pos++;
        }
        String numStr = input.substring(start, pos);
        int result = 0;
        int sign = 1;
        int i = 0;
        if (numStr.charAt(0) == '-') {
            sign = -1;
            i = 1;
        }
        while (i < numStr.length()) {
            result = result * 10 + (numStr.charAt(i) - '0');
            i++;
        }
        return result * sign;
    }

    public boolean parseBoolean() {
        skipWhitespace();
        if (input.charAt(pos) == 't') {
            pos += 4; // true
            return true;
        } else {
            pos += 5; // false
            return false;
        }
    }

    public void parseNull() {
        skipWhitespace();
        pos += 4; // null
    }

    public String parseValue() {
        char c = peek();
        if (c == '"') {
            return "STRING:" + parseString();
        } else if (c == '{') {
            return parseObject();
        } else if (c == '[') {
            return parseArray();
        } else if (c == 't' || c == 'f') {
            return "BOOL:" + parseBoolean();
        } else if (c == 'n') {
            parseNull();
            return "NULL";
        } else {
            return "NUM:" + parseNumber();
        }
    }

    public String parseObject() {
        expect('{');
        StringBuilder sb = new StringBuilder();
        sb.append("{");
        boolean first = true;
        while (peek() != '}') {
            if (!first) {
                expect(',');
                sb.append(", ");
            }
            first = false;
            String key = parseString();
            expect(':');
            String value = parseValue();
            sb.append(key);
            sb.append("=");
            sb.append(value);
        }
        expect('}');
        sb.append("}");
        return "OBJ:" + sb.toString();
    }

    public String parseArray() {
        expect('[');
        StringBuilder sb = new StringBuilder();
        sb.append("[");
        boolean first = true;
        while (peek() != ']') {
            if (!first) {
                expect(',');
                sb.append(", ");
            }
            first = false;
            sb.append(parseValue());
        }
        expect(']');
        sb.append("]");
        return "ARR:" + sb.toString();
    }

    public static void main(String[] args) {
        // Test 1: Simple string
        JsonParser p1 = new JsonParser("\"hello world\"");
        System.out.println(p1.parseValue());

        // Test 2: Number
        JsonParser p2 = new JsonParser("42");
        System.out.println(p2.parseValue());

        // Test 3: Negative number
        JsonParser p3 = new JsonParser("-17");
        System.out.println(p3.parseValue());

        // Test 4: Boolean values
        JsonParser p4 = new JsonParser("true");
        System.out.println(p4.parseValue());

        JsonParser p5 = new JsonParser("false");
        System.out.println(p5.parseValue());

        // Test 5: Null
        JsonParser p6 = new JsonParser("null");
        System.out.println(p6.parseValue());

        // Test 6: Simple object
        JsonParser p7 = new JsonParser("{\"name\": \"Alice\", \"age\": 30}");
        System.out.println(p7.parseValue());

        // Test 7: Array of numbers
        JsonParser p8 = new JsonParser("[1, 2, 3]");
        System.out.println(p8.parseValue());

        // Test 8: Nested object
        JsonParser p9 = new JsonParser("{\"person\": {\"name\": \"Bob\"}, \"active\": true}");
        System.out.println(p9.parseValue());

        // Test 9: Array of mixed types
        JsonParser p10 = new JsonParser("[\"hello\", 42, true, null]");
        System.out.println(p10.parseValue());

        // Test 10: Escape sequences
        JsonParser p11 = new JsonParser("\"line1\\nline2\"");
        System.out.println(p11.parseValue());

        System.out.println("JSON parser tests complete");
    }
}
