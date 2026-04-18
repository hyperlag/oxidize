public class SwitchYield {
    public static void main(String[] args) {
        // Simple yield blocks with literal values
        int x = 2;
        int result = switch (x) {
            case 1 -> { yield 10; }
            case 2 -> { yield 20; }
            case 3 -> { yield 30; }
            default -> { yield 0; }
        };
        System.out.println(result);

        // Yield with computation before the yield
        int day = 3;
        String name = switch (day) {
            case 1 -> { yield "Monday"; }
            case 2 -> { yield "Tuesday"; }
            case 3 -> {
                String base = "Wednes";
                yield base + "day";
            }
            default -> { yield "Other"; }
        };
        System.out.println(name);

        // Mixed: some expression arms, some block arms
        int score = 85;
        String grade = switch (score / 10) {
            case 10, 9 -> "A";
            case 8 -> {
                String g = "B";
                yield g;
            }
            case 7 -> "C";
            default -> {
                String fallback = "F";
                yield fallback;
            }
        };
        System.out.println(grade);

        // Yield with method calls
        int val = 42;
        String desc = switch (val % 3) {
            case 0 -> {
                String s = String.valueOf(val);
                yield "divisible:" + s;
            }
            case 1 -> {
                int rem = val % 3;
                yield "remainder:" + rem;
            }
            default -> { yield "other"; }
        };
        System.out.println(desc);
    }
}
