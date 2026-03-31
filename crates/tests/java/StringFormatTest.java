public class StringFormatTest {
    public static void main(String[] args) {
        // Basic string format
        String s1 = String.format("Hello, %s!", "World");
        System.out.println(s1);

        // Integer format
        String s2 = String.format("Count: %d", 42);
        System.out.println(s2);

        // Float format
        String s3 = String.format("Pi: %.2f", 3.14159);
        System.out.println(s3);

        // Multiple args
        String s4 = String.format("%s has %d items", "Cart", 5);
        System.out.println(s4);

        // Hex format
        String s5 = String.format("Hex: %x", 255);
        System.out.println(s5);

        // Octal
        String s6 = String.format("Oct: %o", 8);
        System.out.println(s6);

        // Percent literal
        String s7 = String.format("100%%");
        System.out.println(s7);

        // Padded
        String s8 = String.format("[%10s]", "right");
        System.out.println(s8);

        String s9 = String.format("[%-10s]", "left");
        System.out.println(s9);

        // String.join
        String joined = String.join(", ", "a", "b", "c");
        System.out.println("joined = " + joined);

        // printf
        System.out.printf("printf: %s %d%n", "test", 99);
    }
}
