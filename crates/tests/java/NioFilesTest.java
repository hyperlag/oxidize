import java.nio.file.*;
import java.util.List;
import java.util.ArrayList;

public class NioFilesTest {
    public static void main(String[] args) throws Exception {
        // Create a path and write content
        Path p = Paths.get("test_nio.txt");
        Files.writeString(p, "Hello NIO");
        System.out.println("File exists: " + Files.exists(p));
        System.out.println("Is regular file: " + Files.isRegularFile(p));

        // Read it back
        String content = Files.readString(p);
        System.out.println("Content: " + content);

        // File size
        long size = Files.size(p);
        System.out.println("Size: " + size);

        // Write lines
        Path p2 = Paths.get("test_nio_lines.txt");
        ArrayList<String> lines = new ArrayList<>();
        lines.add("alpha");
        lines.add("beta");
        lines.add("gamma");
        Files.write(p2, lines);

        // Read lines back
        List<String> readLines = Files.readAllLines(p2);
        for (String line : readLines) {
            System.out.println(line);
        }

        // Path operations
        Path abs = p.toAbsolutePath();
        System.out.println("Absolute path ends with test_nio.txt: " + abs.toString().endsWith("test_nio.txt"));

        Path fileName = p.getFileName();
        System.out.println("File name: " + fileName);

        // Clean up
        Files.delete(p);
        Files.delete(p2);
        System.out.println("Deleted: " + !Files.exists(p));
    }
}
