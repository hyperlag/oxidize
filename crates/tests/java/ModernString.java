public class ModernString {
    public static void main(String[] args) {
        String s = "  hello  ";
        System.out.println(s.strip());
        System.out.println(s.stripLeading());
        System.out.println(s.stripTrailing());

        String blank = "   ";
        System.out.println(blank.isBlank());
        System.out.println("hi".isBlank());

        System.out.println("ab".repeat(3));
    }
}
