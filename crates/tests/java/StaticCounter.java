public class StaticCounter {
    static int count = 0;

    public static void increment() {
        count++;
    }

    public static void add(int n) {
        count += n;
    }

    public static void main(String[] args) {
        increment();
        increment();
        increment();
        System.out.println(count);
        add(10);
        System.out.println(count);
    }
}
