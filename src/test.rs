use crate::dijkstra::GridPoint;

#[test]
fn test_grid_point() {
    let point1: GridPoint = (1, 2, 3);
    let point2: GridPoint = (1, 2, 3);
    let point3: GridPoint = (1, 2, 4);
    let point4: GridPoint = (2, 2, 3);
    assert_eq!(point1, point2);
    assert_ne!(point1, point3);
    assert_ne!(point2, point3);
    assert_ne!(point1, point4);
}
