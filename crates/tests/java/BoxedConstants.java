public class BoxedConstants {
    public static void main(String[] args) {
        System.out.println(Integer.MAX_VALUE);
        System.out.println(Integer.MIN_VALUE);
        System.out.println(Long.MAX_VALUE);
        System.out.println(Long.MIN_VALUE);
        System.out.println(Integer.toBinaryString(10));
        System.out.println(Integer.toHexString(255));
        System.out.println(Integer.bitCount(7));
        System.out.println(Integer.compare(1, 2));
        System.out.println(Long.compare(5L, 5L));
        System.out.println(Double.compare(Double.NaN, 0.0));
        System.out.println(Double.compare(-0.0, 0.0));
        System.out.println(Long.parseLong("42"));
        System.out.println(Double.parseDouble("3.14") > 3.0);
    }
}
