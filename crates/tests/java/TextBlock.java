public class TextBlock {
    public static void main(String[] args) {
        String simple = """
                hello
                world
                """;
        System.out.print("simple=");
        System.out.print(simple);

        String indented = """
                line1
                    line2
                line3
                """;
        System.out.print("indented=");
        System.out.print(indented);

        String noTrailing = """
                abc
                def""";
        System.out.print("noTrailing=");
        System.out.println(noTrailing);

        System.out.println("done");
    }
}
