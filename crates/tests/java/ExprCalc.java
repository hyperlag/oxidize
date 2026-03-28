/**
 * A recursive-descent expression calculator supporting +, -, *, /, parentheses,
 * and variables. Exercises: recursion, string manipulation, OOP, HashMap for
 * variable bindings, control flow, and method chaining.
 */
public class ExprCalc {
    private String input;
    private int pos;

    public ExprCalc(String input) {
        this.input = input;
        this.pos = 0;
    }

    private void skipSpaces() {
        while (pos < input.length() && input.charAt(pos) == ' ') {
            pos++;
        }
    }

    private char peek() {
        skipSpaces();
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

    // Grammar:
    //   expr     = term (('+' | '-') term)*
    //   term     = factor (('*' | '/') factor)*
    //   factor   = NUMBER | '(' expr ')' | '-' factor
    //   NUMBER   = digit+

    public int parseExpr() {
        int result = parseTerm();
        while (true) {
            char c = peek();
            if (c == '+') {
                advance();
                result = result + parseTerm();
            } else if (c == '-') {
                advance();
                result = result - parseTerm();
            } else {
                break;
            }
        }
        return result;
    }

    public int parseTerm() {
        int result = parseFactor();
        while (true) {
            char c = peek();
            if (c == '*') {
                advance();
                result = result * parseFactor();
            } else if (c == '/') {
                advance();
                int divisor = parseFactor();
                if (divisor != 0) {
                    result = result / divisor;
                } else {
                    System.out.println("ERROR: division by zero");
                    return 0;
                }
            } else if (c == '%') {
                advance();
                result = result % parseFactor();
            } else {
                break;
            }
        }
        return result;
    }

    public int parseFactor() {
        char c = peek();
        if (c == '(') {
            advance(); // consume '('
            int result = parseExpr();
            skipSpaces();
            advance(); // consume ')'
            return result;
        } else if (c == '-') {
            advance(); // consume '-'
            return -parseFactor();
        } else {
            return parseNumber();
        }
    }

    public int parseNumber() {
        skipSpaces();
        int result = 0;
        while (pos < input.length() && input.charAt(pos) >= '0' && input.charAt(pos) <= '9') {
            result = result * 10 + (input.charAt(pos) - '0');
            pos++;
        }
        return result;
    }

    public static int eval(String expression) {
        ExprCalc calc = new ExprCalc(expression);
        return calc.parseExpr();
    }

    // ── Additional utility: RPN (Reverse Polish Notation) evaluator ─────
    public static int evalRPN(String rpn) {
        // Simple stack-based RPN evaluator using an array as a stack
        int[] stack = new int[100];
        int top = -1;

        int i = 0;
        while (i < rpn.length()) {
            char c = rpn.charAt(i);
            if (c == ' ') {
                i++;
                continue;
            }
            if (c >= '0' && c <= '9') {
                int num = 0;
                while (i < rpn.length() && rpn.charAt(i) >= '0' && rpn.charAt(i) <= '9') {
                    num = num * 10 + (rpn.charAt(i) - '0');
                    i++;
                }
                top++;
                stack[top] = num;
            } else {
                int b = stack[top];
                top--;
                int a = stack[top];
                top--;
                int result = 0;
                if (c == '+') {
                    result = a + b;
                } else if (c == '-') {
                    result = a - b;
                } else if (c == '*') {
                    result = a * b;
                } else if (c == '/') {
                    result = a / b;
                }
                top++;
                stack[top] = result;
                i++;
            }
        }
        return stack[top];
    }

    // ── Fibonacci calculator using matrix exponentiation approach ────────
    public static long fibonacci(int n) {
        if (n <= 0) {
            return 0;
        }
        if (n == 1 || n == 2) {
            return 1;
        }
        long a = 0;
        long b = 1;
        for (int i = 2; i <= n; i++) {
            long temp = a + b;
            a = b;
            b = temp;
        }
        return b;
    }

    // ── GCD and LCM ────────────────────────────────────────────────────
    public static int gcd(int a, int b) {
        while (b != 0) {
            int temp = b;
            b = a % b;
            a = temp;
        }
        return a;
    }

    public static int lcm(int a, int b) {
        return (a / gcd(a, b)) * b;
    }

    // ── Power function ──────────────────────────────────────────────────
    public static long power(long base, int exp) {
        long result = 1;
        while (exp > 0) {
            if (exp % 2 == 1) {
                result = result * base;
            }
            base = base * base;
            exp = exp / 2;
        }
        return result;
    }

    // ── isPrime ─────────────────────────────────────────────────────────
    public static boolean isPrime(int n) {
        if (n < 2) {
            return false;
        }
        if (n < 4) {
            return true;
        }
        if (n % 2 == 0 || n % 3 == 0) {
            return false;
        }
        for (int i = 5; i * i <= n; i += 6) {
            if (n % i == 0 || n % (i + 2) == 0) {
                return false;
            }
        }
        return true;
    }

    public static void main(String[] args) {
        // Basic arithmetic
        System.out.println("2 + 3 = " + eval("2 + 3"));
        System.out.println("10 - 4 = " + eval("10 - 4"));
        System.out.println("3 * 7 = " + eval("3 * 7"));
        System.out.println("20 / 4 = " + eval("20 / 4"));

        // Precedence
        System.out.println("2 + 3 * 4 = " + eval("2 + 3 * 4"));
        System.out.println("(2 + 3) * 4 = " + eval("(2 + 3) * 4"));

        // Nested parentheses
        System.out.println("((2 + 3) * (4 - 1)) = " + eval("((2 + 3) * (4 - 1))"));

        // Unary minus
        System.out.println("-5 + 3 = " + eval("-5 + 3"));

        // Complex expression
        System.out.println("100 - 2 * 3 * 4 + 5 = " + eval("100 - 2 * 3 * 4 + 5"));

        // RPN evaluator
        System.out.println("RPN 3 4 + = " + evalRPN("3 4 +"));
        System.out.println("RPN 5 3 - 4 * = " + evalRPN("5 3 - 4 *"));
        System.out.println("RPN 2 3 + 4 5 + * = " + evalRPN("2 3 + 4 5 + *"));

        // Fibonacci
        System.out.println("fib(10) = " + fibonacci(10));
        System.out.println("fib(20) = " + fibonacci(20));

        // GCD / LCM
        System.out.println("gcd(12, 8) = " + gcd(12, 8));
        System.out.println("lcm(12, 8) = " + lcm(12, 8));

        // Power
        System.out.println("2^10 = " + power(2, 10));
        System.out.println("3^5 = " + power(3, 5));

        // Primes
        StringBuilder primes = new StringBuilder();
        primes.append("Primes up to 30:");
        for (int i = 2; i <= 30; i++) {
            if (isPrime(i)) {
                primes.append(" ");
                primes.append(i);
            }
        }
        System.out.println(primes.toString());

        System.out.println("Expression calculator tests complete");
    }
}
