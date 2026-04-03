import java.util.Properties;
import java.io.StringReader;

public class PropertiesTest {
    public static void main(String[] args) throws Exception {
        // Basic get/set
        Properties p = new Properties();
        p.setProperty("name", "Alice");
        p.setProperty("age", "30");

        System.out.println(p.getProperty("name"));
        System.out.println(p.getProperty("age"));
        System.out.println(p.getProperty("missing", "default"));
        System.out.println("size=" + p.size());
        System.out.println("has_name=" + p.containsKey("name"));
        System.out.println("empty=" + p.isEmpty());

        // Load from string (simulates load(new StringReader(...)))
        Properties p2 = new Properties();
        p2.load(new StringReader("host=localhost\nport=8080\n# comment\n!also comment"));
        System.out.println(p2.getProperty("host"));
        System.out.println(p2.getProperty("port"));
        System.out.println("p2size=" + p2.size());
    }
}
