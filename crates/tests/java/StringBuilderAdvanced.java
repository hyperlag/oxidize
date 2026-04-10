public class StringBuilderAdvanced {
    public static void main(String[] args) {
        // reverse
        StringBuilder sb = new StringBuilder("Hello World");
        sb.reverse();
        System.out.println(sb.toString());  // dlroW olleH

        // insert at position 5
        sb = new StringBuilder("Hello World");
        sb.insert(5, "!!!");
        System.out.println(sb.toString());  // Hello!!! World

        // delete chars [5, 11) removes " World"
        sb = new StringBuilder("Hello World");
        sb.delete(5, 11);
        System.out.println(sb.toString());  // Hello

        // replace chars [6, 11) with "Java"
        sb = new StringBuilder("Hello World");
        sb.replace(6, 11, "Java");
        System.out.println(sb.toString());  // Hello Java

        // indexOf
        sb = new StringBuilder("Hello World");
        System.out.println(sb.indexOf("World"));  // 6

        // lastIndexOf
        sb = new StringBuilder("abcabc");
        System.out.println(sb.lastIndexOf("bc"));  // 4
    }
}
