import java.util.StringJoiner;

public class StringJoinerTest {
    public static void main(String[] args) {
        // 31A: String.formatted() — Java 15+ instance method form of String.format()
        String name = "World";
        String greeting = "Hello, %s!".formatted(name);
        System.out.println(greeting);                        // Hello, World!

        String pi = "Pi is %.2f".formatted(3.14159);
        System.out.println(pi);                              // Pi is 3.14

        // 31B: StringJoiner — delimiter only
        StringJoiner sj = new StringJoiner(", ");
        sj.add("a");
        sj.add("b");
        sj.add("c");
        System.out.println(sj.toString());                   // a, b, c

        // StringJoiner — delimiter + prefix + suffix
        StringJoiner sj2 = new StringJoiner(", ", "[", "]");
        sj2.add("x");
        sj2.add("y");
        System.out.println(sj2.toString());                  // [x, y]

        // StringJoiner — empty value
        StringJoiner sj3 = new StringJoiner(", ", "{", "}");
        sj3.setEmptyValue("empty");
        System.out.println(sj3.toString());                  // empty

        // StringJoiner — merge
        StringJoiner sj4 = new StringJoiner("-");
        sj4.add("1");
        sj4.add("2");
        StringJoiner sj5 = new StringJoiner("-");
        sj5.add("3");
        sj5.add("4");
        sj4.merge(sj5);
        System.out.println(sj4.toString());                  // 1-2-3-4

        // StringJoiner — length
        StringJoiner sj6 = new StringJoiner(", ", "[", "]");
        sj6.add("hello");
        System.out.println(sj6.length());                    // 7  ([hello])
    }
}
