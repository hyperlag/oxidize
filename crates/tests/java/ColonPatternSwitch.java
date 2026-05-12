class Token {
    String kind;
    int value;
    Token(String k, int v) { this.kind = k; this.value = v; }
}

public class ColonPatternSwitch {
    // Basic colon-form pattern switch with break and default arm.
    static String describe(Token t) {
        switch (t) {
            case Token x:
                return x.kind + "=" + x.value;
            default:
                return "unknown";
        }
    }

    // Colon-form switch statement (not expression) storing result in a local.
    static int doubled(Token t) {
        int result;
        switch (t) {
            case Token x:
                result = x.value * 2;
                break;
            default:
                result = -1;
                break;
        }
        return result;
    }

    // Colon-form with multi-statement arm body.
    static String summary(Token t) {
        String msg;
        switch (t) {
            case Token x:
                String prefix = x.kind.toUpperCase();
                msg = prefix + ":" + x.value;
                break;
            default:
                msg = "none";
                break;
        }
        return msg;
    }

    public static void main(String[] args) {
        System.out.println(describe(new Token("add", 1)));   // add=1
        System.out.println(describe(new Token("mul", 7)));   // mul=7
        System.out.println(doubled(new Token("x", 5)));      // 10
        System.out.println(doubled(new Token("y", 3)));      // 6
        System.out.println(summary(new Token("sub", 4)));    // SUB:4
    }
}
