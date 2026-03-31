public class SystemTest {
    public static void main(String[] args) {
        // currentTimeMillis - just check it's positive
        long millis = System.currentTimeMillis();
        System.out.println("millis > 0: " + (millis > 0));

        // nanoTime - just check it's positive
        long nanos = System.nanoTime();
        System.out.println("nanos > 0: " + (nanos > 0));

        // lineSeparator
        String sep = System.lineSeparator();
        System.out.println("lineSep length: " + sep.length());

        // getProperty
        String fileSep = System.getProperty("file.separator");
        System.out.println("fileSep = " + fileSep);

        String osName = System.getProperty("os.name");
        System.out.println("osName empty: " + osName.isEmpty());

        // getProperty with default
        String missing = System.getProperty("nonexistent.key", "default_val");
        System.out.println("missing = " + missing);
    }
}
