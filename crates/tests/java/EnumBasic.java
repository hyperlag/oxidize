public class EnumBasic {
    enum Color { RED, GREEN, BLUE }

    public static void main(String[] args) {
        Color c = Color.GREEN;
        System.out.println(c.name());
        System.out.println(c.ordinal());
        System.out.println(c);
        System.out.println(Color.values().length);
    }
}
