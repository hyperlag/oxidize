import java.io.*;
import java.util.Scanner;

public class ScannerTest {
    public static void main(String[] args) throws Exception {
        // Write a test file to scan
        PrintWriter pw = new PrintWriter("test_scanner.txt");
        pw.println("Hello World");
        pw.println("42");
        pw.println("3.14");
        pw.println("Last line");
        pw.close();

        // Read with Scanner from file
        Scanner sc = new Scanner(new File("test_scanner.txt"));
        while (sc.hasNextLine()) {
            System.out.println(sc.nextLine());
        }
        sc.close();

        // Clean up
        new File("test_scanner.txt").delete();
        System.out.println("Done");
    }
}
