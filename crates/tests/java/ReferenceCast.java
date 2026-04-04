public class ReferenceCast {
    static String wrap(String s) {
        return (String) s;
    }

    public static void main(String[] args) {
        String a = (String) "hello";
        System.out.println(a);
        System.out.println(a.length());
        System.out.println(wrap("world"));
    }
}
