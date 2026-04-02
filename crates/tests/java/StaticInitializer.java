public class StaticInitializer {
    static int value;
    static int doubled;

    static {
        value = 42;
        doubled = value * 2;
    }

    public static int getValue() {
        return value;
    }

    public static void main(String[] args) {
        System.out.println(getValue());
        System.out.println(doubled);
    }
}
