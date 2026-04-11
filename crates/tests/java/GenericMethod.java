class GenericMethod {
    static <T extends Comparable<T>> T maxOf(T a, T b) {
        if (a.compareTo(b) >= 0) return a;
        return b;
    }

    static <T> T identity(T x) {
        return x;
    }

    public static void main(String[] args) {
        System.out.println(maxOf("apple", "banana"));
        System.out.println(maxOf("zebra", "ant"));
        System.out.println(identity("hello"));
        System.out.println(identity(42));
    }
}
